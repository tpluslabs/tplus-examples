use anyhow::Result;
use light_client::LightClientHandler;
use overlay::utils::setup_overlay_from_commandline;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let secret_key = mocks::get_node_secret();
    let (comms_receiver, broadcast_tx, peers, mut handles) =
        setup_overlay_from_commandline(secret_key).await?;
    let (oneshot_send, oneshot_rx) = tokio::sync::oneshot::channel();

    let light_client_task =
        tokio::spawn(async move { light_client::helios::run(oneshot_rx).await.map(|_| ()) });

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
