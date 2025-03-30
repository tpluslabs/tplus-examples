use crate::{
    encryption::{self, ChiperWrapper},
    error::OverlayError,
    macros::helper::make_continue,
    message::{
        MaybeEncrypted, OverlayHeader, OverlayMessage, OverlayMessageType, OverlayOnboard,
        OverlayPacket,
    },
    P2PTransportRecvMiddleman, P2PTransportSendMiddleman, GLOB_CHANNEL_BUFFER, NONCE_WINDOW,
};
use secp256k1::{ecdsa, Message, PublicKey, Secp256k1};
use sha2::{Digest, Sha256};
use tokio::{
    runtime::Handle,
    sync::{broadcast::error::RecvError, mpsc::Sender},
};

pub struct P2PSessionData {
    pub session: i64,
    pub peer: Vec<u8>,
    pub chiper: encryption::ChiperWrapper,
}

/// Middleware between raw layer and app layer. Likely should get abstracted too.
pub struct P2PConnectionManager {
    /// nonces recently used by the peer.
    pub cached_nonces: Vec<i64>,
    /// our own nonce
    pub nonce: i64,
    /// the peer's nonce according to our local view.
    pub peer_nonce: i64,
    /// Secret key associated with the node.
    pub secret: secp256k1::SecretKey,
    //pub shared_secret: Option<secp256k1::SecretKey>,
    pub data: Option<P2PSessionData>,
}

enum InternalMessage {
    Inbound(OverlayPacket),
    Outbound(OverlayMessage),
}

impl P2PConnectionManager {
    pub fn new(
        secret: secp256k1::SecretKey,
        //    _shared_secret: Option<secp256k1::SecretKey>
    ) -> Self {
        Self {
            cached_nonces: vec![],
            nonce: 0,
            peer_nonce: 0,
            secret,
            //shared_secret,
            data: None,
        }
    }

    fn handle_check_nonce(&mut self, packet_nonce: i64) -> anyhow::Result<()> {
        let peer_nonce = self.peer_nonce;

        if peer_nonce + NONCE_WINDOW < packet_nonce || peer_nonce - NONCE_WINDOW > packet_nonce {
            return Err(OverlayError::InvalidNonce(peer_nonce, packet_nonce).into());
        }

        if self.cached_nonces.contains(&packet_nonce) {
            return Err(OverlayError::ExpiredNonce(packet_nonce).into());
        }

        self.cached_nonces
            .retain(|&cached| cached + NONCE_WINDOW >= peer_nonce);

        Ok(())
    }

    pub async fn queue<S, R>(
        &mut self,
        mut comms_receiver: tokio::sync::broadcast::Receiver<OverlayMessage>,
        mut connection: S,
        mut incoming: R,
        sender: Sender<OverlayMessage>,
    ) -> anyhow::Result<()>
    where
        S: P2PTransportSendMiddleman,
        // NB: incoming must not outlive the task
        R: P2PTransportRecvMiddleman + Send + 'static,
    {
        tracing::debug!("started queue service on p2p connection");
        let (tx, mut rx) = tokio::sync::mpsc::channel::<InternalMessage>(GLOB_CHANNEL_BUFFER);
        let handle = Handle::current();

        let key = self.secret;
        let pubkey = key.public_key(&Secp256k1::new()).serialize().to_vec();

        // NB: we need to adapt based on the dstack-guest interface that the community agrees upon.
        // Currently our tsm-quote-generation lib takes in any bytes and does the hashing, but some other
        // impls might require the hashed report data directly. The latter approach is probably better
        // because it lets you choose the hashing algo.
        let quote = mocks::get_quote(&pubkey).await?;
        // let self_want_shared = self.shared_secret.is_none(); NB: we are not using this field anyways

        // NB: this isn't actually encrypted since it's the handshake message.
        let (send_quote, local_session_key) = OverlayPacket::from_quote(&pubkey, quote, false)?;

        // NB: error propagation here is correct, we need to close the task.
        connection
            .connection_send_message(bincode::serialize(&send_quote)?)
            .await?;

        let cloned = tx.clone();
        handle.spawn(async move {
            loop {
                match comms_receiver.recv().await {
                    Ok(message) => {
                        if let Err(_) = cloned
                            .clone()
                            .send(InternalMessage::Outbound(message))
                            .await
                        {
                            tracing::error!(
                            "Queue task dropped unexpectedly. Need to re-establish the connection"
                        );

                            break;
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        tracing::warn!("lagged by {n}")
                    }
                    Err(_) => {
                        tracing::error!("channel closed")
                    }
                }
            }
        });

        handle.spawn(async move {
            while let Ok(Some(bytes)) = incoming.incoming_requests().await {
                // we discard malformed messages
                if let Ok(packet) = bincode::deserialize::<OverlayPacket>(&bytes) {
                    if let Err(_) = tx.clone().send(InternalMessage::Inbound(packet)).await {
                        // receiver dropped, in prod it means that we need to log this and
                        // try to re-establish the connection.
                        tracing::error!(
                            "Queue task dropped unexpectedly. Need to re-establish the connection"
                        );

                        break;
                    }
                }
            }
        });

        while let Some(internal_msg) = rx.recv().await {
            match internal_msg {
                InternalMessage::Inbound(packet) => {
                    // need to forward to comms channel
                    match &packet.header {
                        // we already have a safe encrypted communication channel.
                        Some(header) => {
                            if self.data.is_none() {
                                //return Err(crate::error::OverlayError::MalformedOnboard.into())
                                // keeping above to reference in the todo log system that we need to
                                // notify but not propagate to resist dos.
                                continue;
                            }
                            let local_session_data = self.data.as_ref().unwrap();

                            let h_payload: [u8; 32] = {
                                let payload = packet.to_payload().unwrap();
                                sha2::Sha256::digest(&payload).try_into().unwrap()
                            };
                            let signature_deser =
                                make_continue!(ecdsa::Signature::from_compact(&header.signature));
                            let secp = Secp256k1::new();
                            let msg = Message::from_digest(h_payload);
                            let public_key = make_continue!(PublicKey::from_slice(&packet.pubkey));
                            make_continue!(secp.verify_ecdsa(&msg, &signature_deser, &public_key)); // we probably should notify overlay
                                                                                                    // of suspect behavior.

                            // NB: here we want to actually propagate the error since it means that the peer is not synced.
                            // they'll have to re-establish the connection.
                            if header.session_key != local_session_data.session {
                                return Err(crate::error::OverlayError::InvalidSessionKey(
                                    local_session_data.session,
                                    header.session_key,
                                )
                                .into());
                            }

                            self.handle_check_nonce(header.nonce)?;

                            match &packet.message.message {
                                MaybeEncrypted::EncryptedP2P(to_decrypt) => {
                                    let decrypted_message = self
                                        .data
                                        .as_ref()
                                        .unwrap()
                                        .chiper
                                        .get_decrypted_message(&to_decrypt)?;

                                    let _ = sender
                                        .send(OverlayMessage::new_p2p_encrypted(
                                            Some(vec![self.data.as_ref().unwrap().peer.clone()]),
                                            decrypted_message,
                                        ))
                                        .await;
                                } // NB: full implementation reserves other messages
                            }

                            self.peer_nonce += 1;
                        }
                        // very first message
                        None => {
                            if self.data.is_some() {
                                // we actually don't want to error here since it's vulnerable to reply by malicious
                                // host since quotes don't carry nonces. We just ignore the message and potentially
                                // notify other nodes that this peer is not behaving as expected.
                                continue;
                            }

                            // NB: this isn't actually encrypted
                            let MaybeEncrypted::EncryptedP2P(decrypted_message) =
                                packet.message.message;

                            let message_deser: OverlayMessageType =
                                bincode::deserialize(&decrypted_message)?;

                            let OverlayMessageType::Onboard(OverlayOnboard {
                                quote,
                                session,
                                want_shared: _,
                            }) = message_deser
                            else {
                                return Err(crate::error::OverlayError::GotNoQuote.into());
                            };

                            // NB: any deterministic function works good here.
                            let session_key = if session < local_session_key {
                                local_session_key
                            } else {
                                session
                            };

                            let quote_verification = mocks::verify_quote(&quote, &packet.pubkey);
                            if !quote_verification.await.is_valid {
                                return Err(crate::error::OverlayError::InvaildQuote(quote).into());
                            }

                            self.data = Some(P2PSessionData {
                                session: session_key,
                                peer: packet.pubkey.clone(),
                                chiper: ChiperWrapper::new(
                                    &self.secret.secret_bytes(),
                                    &packet.pubkey,
                                )?,
                            });
                        }
                    }
                }

                InternalMessage::Outbound(mut message) => {
                    if let Some(targets) = &message.targets {
                        // if the app specified targets and if our connection's peer is not
                        // in that array then we ignore the outbound request.
                        if !targets.contains(&self.data.as_ref().unwrap().peer) {
                            continue;
                        }
                    }

                    {
                        let MaybeEncrypted::EncryptedP2P(to_encrypt) = message.message;
                        // NB: if it's EncryptedP2P we want to encrypt it to the peer else we leave it up to the app.
                        let chiper = &self.data.as_ref().unwrap().chiper;
                        let encrypted = chiper.get_encrypted_message(&to_encrypt)?;
                        message.message = MaybeEncrypted::EncryptedP2P(encrypted);
                    }

                    // need to send to connection
                    let mut packet = OverlayPacket {
                        header: Some(OverlayHeader {
                            nonce: self.nonce,
                            session_key: self.data.as_ref().unwrap().session,
                            signature: vec![],
                        }),
                        pubkey: pubkey.clone(),
                        message,
                    };
                    let h_payload: [u8; 32] = Sha256::digest(&packet.to_payload().unwrap())
                        .try_into()
                        .unwrap();
                    let signature = {
                        let secp = Secp256k1::new();
                        let message = Message::from_digest(h_payload);
                        let signature = secp.sign_ecdsa(&message, &key);
                        signature.serialize_compact()
                    };
                    packet.add_signature(signature.to_vec());
                    connection
                        .connection_send_message(bincode::serialize(&packet)?)
                        .await?;

                    self.nonce += 1;
                }
            }
        }

        Ok(())
    }
}
