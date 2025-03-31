use anyhow::Result;
use light_client::LightClientHandler;
use overlay::utils::setup_overlay_from_config;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use warp::Filter;

mod config_server {
    use super::*;

    #[derive(Deserialize, Clone, Debug)]
    pub struct NodeConfig {
        pub peers: Vec<String>,
        pub port: u16,
        pub execution_rpc: String,
    }

    pub async fn load_config_server(config_oneshot_sender: oneshot::Sender<NodeConfig>) {
        let config_sender = Arc::new(Mutex::new(Some(config_oneshot_sender)));
        let config_sender_filter = warp::any().map({
            let config_sender = config_sender.clone();
            move || config_sender.clone()
        });

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let shutdown_sender = Arc::new(Mutex::new(Some(shutdown_tx)));
        let shutdown_sender_filter = warp::any().map({
            let shutdown_sender = shutdown_sender.clone();
            move || shutdown_sender.clone()
        });

        let setup = warp::path("setup")
            .and(warp::post())
            .and(warp::body::json())
            .and(config_sender_filter)
            .and(shutdown_sender_filter)
            .and_then(
                |config: NodeConfig,
                 config_sender: Arc<Mutex<Option<oneshot::Sender<NodeConfig>>>>,
                 shutdown_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>| async move {
                    if let Some(sender) = config_sender.lock().await.take() {
                        let _ = sender.send(config);
                    }

                    // NB: we shut down the config API once the configuration is sent.
                    if let Some(shutdown) = shutdown_sender.lock().await.take() {
                        let _ = shutdown.send(());
                    }
                    Ok::<_, Infallible>(warp::reply::json(
                        &serde_json::json!({ "status": "success" }),
                    ))
                },
            );

        let (_addr, server) =
            warp::serve(setup).bind_with_graceful_shutdown(([0, 0, 0, 0], 40080), async {
                shutdown_rx.await.ok();
            });
        server.await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let (config_tx, config_rx) = oneshot::channel::<config_server::NodeConfig>();
    let server_handle = tokio::spawn(async move {
        config_server::load_config_server(config_tx).await;
    });
    let config = config_rx.await?;
    server_handle.await?; // this should have already shut down
    tracing::info!("received configuration: {:?}", config);

    let secret_key = mocks::get_node_secret();
    let (comms_receiver, broadcast_tx, peers, mut handles) =
        setup_overlay_from_config(secret_key, config.peers, config.port).await?;
    let (oneshot_send, oneshot_rx) = tokio::sync::oneshot::channel();

    let light_client_task = tokio::spawn(async move {
        light_client::helios::run(oneshot_rx, config.execution_rpc)
            .await
            .map(|_| ())
    });

    let shared_secret = if peers.is_empty() {
        tracing::info!("No peers provided, bootstrapping");
        Some(mocks::get_node_secret().secret_bytes().to_vec())
    } else {
        tracing::info!("Got peers, skipping bootstrapping");
        None
    };

    let solver = LightClientHandler::new(
        comms_receiver,
        broadcast_tx.clone(),
        oneshot_send,
        shared_secret,
    );
    let solver_task = tokio::spawn(async move { solver.handle_messages().await.map(|_| ()) });

    handles.push(solver_task);
    handles.push(light_client_task);

    for handle in handles {
        handle.await.unwrap().unwrap();
    }
    Ok(())
}
