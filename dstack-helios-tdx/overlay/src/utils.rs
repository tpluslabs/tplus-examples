use crate::message::OverlayMessage;
use crate::quic::QUICTransport;
use crate::{P2PTransportLayer, GLOB_CHANNEL_BUFFER};
use secp256k1::SecretKey;
use std::net::{Ipv4Addr, SocketAddr};
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

pub async fn setup_overlay_from_config(
    secret_key: SecretKey,
    peers: Vec<String>,
    listen_port: u16,
) -> anyhow::Result<(
    Receiver<OverlayMessage>,
    Sender<OverlayMessage>,
    Vec<SocketAddr>,
    Vec<JoinHandle<anyhow::Result<()>>>,
)> {
    let peers: Vec<SocketAddr> = peers
        .iter()
        .map(|addr| addr.parse().expect("invalid address"))
        .collect();

    let (comms_sender, comms_receiver) = tokio::sync::mpsc::channel(GLOB_CHANNEL_BUFFER);
    let (broadcast_tx, _) = tokio::sync::broadcast::channel(GLOB_CHANNEL_BUFFER);

    let handles = QUICTransport::forward_messages(
        secret_key,
        (Ipv4Addr::UNSPECIFIED, listen_port).into(),
        peers.iter().copied().collect(),
        comms_sender,
        broadcast_tx.clone(),
    )
    .await?;

    Ok((comms_receiver, broadcast_tx, peers, handles))
}
