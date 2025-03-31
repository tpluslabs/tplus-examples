//! Overlay networking layer with abstracted transport.
//!
//! Any transport layer that implements the [`P2PTransportLayer`] can work with the overlay. The [`P2PTransportLayer::forward_messages`]
//! method is the entry point for spawning the overlay and will return an array of the join handles for the futures being executed
//! on tokio's threads pool. `forward_messages` requires:
//! - an owned copy of the secret key associated to the node. This will be used to sign message headers.
//! - an address to listen requests on.
//! - an array of peers we want to connect to. No discovery.
//! - a sender for the comms channel to send messages from the overlay to the app.
//! - a sender of the comms broadcasting channel, used to generate receivers within the overlay
//! to receive messages from the comms channel since we need multiple receivers. Theoretically
//! we could also collapse the comms broadcast and comms channels together into a single broadcast.
//!
//! Note that for the P2P connection manager to work you'll also need to pass sender and receiver
//! wrappers that implement [`P2PTransportSendMiddleman`] and [`P2PTransportRecvMiddleman`]. They
//! are purposefully split to enable for more specific ownership systems.
//!

use secp256k1::SecretKey;
use std::net::SocketAddr;
use tokio::{runtime::Handle, sync::mpsc::Sender, task::JoinHandle};
mod encryption;
mod error;
pub mod macros;
pub mod message;
pub mod p2p;

// NB: on stripped down implemenation this is the only transport.
#[cfg(feature = "quic")]
pub mod quic;

pub mod tcp;

pub mod utils;

pub const GLOB_CHANNEL_BUFFER: usize = 20000;
pub const NONCE_WINDOW: i64 = 10;

#[async_trait::async_trait]
pub trait P2PTransportLayer
where
    Self: 'static,
{
    type Message: Send;
    type ServeContext;
    type ConnectContext;

    /// Connects to peers.
    async fn connect(
        listener: SocketAddr,
    ) -> anyhow::Result<(Self::ConnectContext, Self::ServeContext)>;

    /// Connects to peers. Needs to return ownership to the comms channel receiver.
    async fn connect_peer(
        secret_key: SecretKey,
        //shared_secret: Option<SecretKey>,
        ctx: Self::ConnectContext,
        peers: Vec<SocketAddr>,
        sender: Sender<Self::Message>,
        comms_broadcast: tokio::sync::broadcast::Sender<Self::Message>,
    ) -> anyhow::Result<()>;

    /// Serve incoming requests.
    async fn serve(
        secret_key: SecretKey,
        //shared_secret: Option<SecretKey>,
        ctx: Self::ServeContext,
        sender: Sender<Self::Message>,
        receiver: tokio::sync::broadcast::Sender<Self::Message>,
    ) -> anyhow::Result<()>;

    /// Entrypoint. Spawns the p2p overlay task that forwards incoming messages. Returns all the threads (running the "sub-tasks") we want to run.
    async fn forward_messages(
        secret_key: SecretKey,
        //shared_secret: Option<SecretKey>,
        listener: SocketAddr,
        peers: Vec<SocketAddr>,
        sender: Sender<Self::Message>,
        comms_broadcast: tokio::sync::broadcast::Sender<Self::Message>,
    ) -> anyhow::Result<Vec<JoinHandle<anyhow::Result<()>>>> {
        let (connect_ctx, serve_ctx) = Self::connect(listener).await?;
        let handle = Handle::current();
        let join_network = handle.spawn(Self::connect_peer(
            secret_key,
            connect_ctx,
            peers,
            sender.clone(),
            comms_broadcast.clone(),
        ));
        let serve = handle.spawn(Self::serve(secret_key, serve_ctx, sender, comms_broadcast));
        Ok(vec![join_network, serve])
    }
}

#[async_trait::async_trait]
pub trait P2PTransportSendMiddleman {
    async fn connection_send_message(&mut self, message: Vec<u8>) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
pub trait P2PTransportRecvMiddleman {
    async fn incoming_requests(&mut self) -> anyhow::Result<Option<Vec<u8>>>;
}
