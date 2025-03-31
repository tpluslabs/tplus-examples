// uses the tokio utils framing abstraction. there's really no difference tbh but currently have it
// here to exepriment with the abstraction.
use crate::{
    message::OverlayMessage, p2p::P2PConnectionManager, P2PTransportLayer,
    P2PTransportRecvMiddleman, P2PTransportSendMiddleman,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::{
    net::{TcpListener, TcpStream},
    runtime::Handle,
    sync::mpsc::Sender,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub struct TCPTransport;
pub struct TCPEndpoint;

impl TCPEndpoint {
    async fn connect_to(
        &self,
        peer: &SocketAddr,
    ) -> anyhow::Result<(TCPTransportConnection, TCPTransportIncomingConnection)> {
        let stream = TcpStream::connect(peer).await?;
        let framed = Framed::new(stream, LengthDelimitedCodec::new());
        let (writer, reader) = framed.split();
        Ok((
            TCPTransportConnection { writer },
            TCPTransportIncomingConnection { reader },
        ))
    }
}

pub struct TCPTransportConnection {
    writer: futures::stream::SplitSink<Framed<TcpStream, LengthDelimitedCodec>, Bytes>,
}
pub struct TCPTransportIncomingConnection {
    reader: futures::stream::SplitStream<Framed<TcpStream, LengthDelimitedCodec>>,
}

#[async_trait]
impl P2PTransportLayer for TCPTransport {
    type Message = OverlayMessage;
    type ConnectContext = TCPEndpoint;
    type ServeContext = TcpListener;

    async fn connect(
        listener: SocketAddr,
    ) -> anyhow::Result<(Self::ConnectContext, Self::ServeContext)> {
        let listener = TcpListener::bind(listener).await?;
        let endpoint = TCPEndpoint;
        Ok((endpoint, listener))
    }

    async fn connect_peer(
        secret_key: secp256k1::SecretKey,
        //shared_secret: Option<secp256k1::SecretKey>,
        ctx: Self::ConnectContext,
        peers: Vec<SocketAddr>,
        sender: Sender<Self::Message>,
        comms_broadcast: tokio::sync::broadcast::Sender<Self::Message>,
    ) -> anyhow::Result<()>
    {
        let handle = Handle::current();
        for peer in peers {
            let comms_broadcast = comms_broadcast.clone();
            let comms_sender = sender.clone();
            let (connection, incoming) = ctx.connect_to(&peer).await?;
            println!("Onboarding to peer {}", peer);
            handle.spawn(async move {
                let rx = comms_broadcast.subscribe();
                let connection_wrapper = connection;
                let recv_wrapper = incoming;
                let _ = P2PConnectionManager::new(secret_key)
                    .queue::<TCPTransportConnection, TCPTransportIncomingConnection>(
                        rx,
                        connection_wrapper,
                        recv_wrapper,
                        comms_sender,
                    )
                    .await;
            });
        }
        Ok(())
    }

    async fn serve(
        secret_key: secp256k1::SecretKey,
        //shared_secret: Option<secp256k1::SecretKey>,
        ctx: Self::ServeContext,
        sender: Sender<Self::Message>,
        comms_broadcast: tokio::sync::broadcast::Sender<Self::Message>,
    ) -> anyhow::Result<()>
    {
        let handle = Handle::current();

        loop {
            let (stream, addr) = ctx.accept().await?;
            println!("Accepted connection from {}", addr);
            let framed = Framed::new(stream, LengthDelimitedCodec::new());
            let (writer, reader) = framed.split();
            let comms_broadcast = comms_broadcast.clone();
            let comms_sender = sender.clone();
            handle.spawn(async move {
                let rx = comms_broadcast.subscribe();
                let connection_wrapper = TCPTransportConnection { writer };
                let recv_wrapper = TCPTransportIncomingConnection { reader };
                let _ = P2PConnectionManager::new(secret_key)
                    .queue::<TCPTransportConnection, TCPTransportIncomingConnection>(
                        rx,
                        connection_wrapper,
                        recv_wrapper,
                        comms_sender,
                    )
                    .await;
            });
        }
    }
}

#[async_trait]
impl P2PTransportSendMiddleman for TCPTransportConnection {
    async fn connection_send_message(&mut self, message: Vec<u8>) -> anyhow::Result<()> {
        self.writer.send(Bytes::from(message)).await?;
        Ok(())
    }
}

#[async_trait]
impl P2PTransportRecvMiddleman for TCPTransportIncomingConnection {
    async fn incoming_requests(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
        if let Some(result) = self.reader.next().await {
            Ok(Some(result?.to_vec()))
        } else {
            Ok(None)
        }
    }
}
