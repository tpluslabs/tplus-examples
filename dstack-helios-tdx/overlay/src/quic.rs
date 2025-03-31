use crate::{
    message::OverlayMessage, p2p::P2PConnectionManager, P2PTransportLayer,
    P2PTransportRecvMiddleman, P2PTransportSendMiddleman,
};
use async_trait::async_trait;
use bytes::Bytes;
use qp2p::{Connection, ConnectionIncoming, Endpoint, IncomingConnections, WireMsg};
use std::net::SocketAddr;
use tokio::{runtime::Handle, sync::mpsc::Sender};

pub struct QUICTransport;

#[async_trait]
impl P2PTransportLayer for QUICTransport {
    type Message = OverlayMessage;
    type ConnectContext = Endpoint;
    type ServeContext = IncomingConnections;

    async fn connect(
        listener: SocketAddr,
    ) -> anyhow::Result<(Self::ConnectContext, Self::ServeContext)> {
        let (node, incoming_conns) = Endpoint::builder()
            .addr(listener)
            .idle_timeout(60 * 60 * 1_000 /* 3600s = 1h */)
            .max_concurrent_uni_streams(1000)
            .max_concurrent_bidi_streams(1000)
            .server()?;

        Ok((node, incoming_conns))
    }

    async fn connect_peer(
        secret_key: secp256k1::SecretKey,
        ctx: Self::ConnectContext,
        peers: Vec<SocketAddr>,
        sender: Sender<Self::Message>,
        comms_broadcast: tokio::sync::broadcast::Sender<Self::Message>,
    ) -> anyhow::Result<()> {
        // we could speed this up fwiw.
        let handle = Handle::current();

        for peer in peers {
            let comms_broadcast = comms_broadcast.clone();
            let comms_sender = sender.clone();

            println!("connecting to peer {}", peer);
            let (connection, incoming) = ctx.connect_to(&peer).await.unwrap();
            println!("connected to peer {}", peer);

            handle.spawn(async move {
                let rx = comms_broadcast.subscribe();
                let connection_wrapper = QUICTransportConnection { connection };
                let recv_wrapper = QUICTransportIncomingConnection { incoming };
                let r = P2PConnectionManager::new(secret_key)
                    .queue::<QUICTransportConnection, QUICTransportIncomingConnection>(
                        rx,
                        connection_wrapper,
                        recv_wrapper,
                        comms_sender,
                    )
                    .await;
                tracing::error!(
                    "queue stopped serving request from joined peer, any attempts to reserve sender now will fail {:?}",
                    r
                )
            });
        }

        Ok(())
    }

    async fn serve(
        secret_key: secp256k1::SecretKey,
        //shared_secret: Option<secp256k1::SecretKey>,
        mut ctx: Self::ServeContext,
        sender: Sender<Self::Message>,
        comms_broadcast: tokio::sync::broadcast::Sender<Self::Message>,
    ) -> anyhow::Result<()> {
        let handle = Handle::current();
        while let Some((connection, incoming)) = ctx.next().await {
            let comms_broadcast = comms_broadcast.clone();
            let comms_sender = sender.clone();
            // we use a dedicated task for each connection
            handle.spawn(async move {
                let rx = comms_broadcast.subscribe();
                let connection_wrapper = QUICTransportConnection { connection };
                let recv_wrapper = QUICTransportIncomingConnection { incoming };
                let r = P2PConnectionManager::new(secret_key)
                    .queue::<QUICTransportConnection, QUICTransportIncomingConnection>(
                        rx,
                        connection_wrapper,
                        recv_wrapper,
                        comms_sender,
                    )
                    .await;
                tracing::error!(
                    "queue stopped serving, any attempts to reserve sender now will fail {:?}",
                    r
                )
            });
        }

        Ok(())
    }
}

pub struct QUICTransportIncomingConnection {
    incoming: ConnectionIncoming,
}

pub struct QUICTransportConnection {
    connection: Connection,
}

#[async_trait]
impl P2PTransportSendMiddleman for QUICTransportConnection {
    async fn connection_send_message(&mut self, message: Vec<u8>) -> anyhow::Result<()> {
        self.connection
            .send((Bytes::new(), Bytes::new(), Bytes::from(message)))
            .await?;
        Ok(())
    }
}

#[async_trait]
impl P2PTransportRecvMiddleman for QUICTransportIncomingConnection {
    async fn incoming_requests(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
        if let Some(WireMsg((_, _, bytes))) = self.incoming.next().await? {
            Ok(Some(bytes.to_vec()))
        } else {
            Ok(None)
        }
    }
}
