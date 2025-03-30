//! This is currently just a mock, it returns the channel to be inferred in the comms task
//! else the task will halt and comms drops.

pub mod helios;

use std::time::Duration;

use overlay::message::{MaybeEncrypted, NotifySharedSecret, OverlayMessage, OverlayMessageType};
use tokio::sync::mpsc::Receiver;

pub struct LightClientHandler {
    /// sends messages to the overlay.
    pub overlay_broadcast_tx: tokio::sync::broadcast::Sender<OverlayMessage>,
    secret: Option<Vec<u8>>,
    receiver: Receiver<OverlayMessage>,
    oneshot_sender: Option<tokio::sync::oneshot::Sender<Vec<u8>>>,
}

impl LightClientHandler {
    pub fn new(
        receiver: Receiver<OverlayMessage>,
        overlay_broadcast_tx: tokio::sync::broadcast::Sender<OverlayMessage>,
        oneshot_sender: tokio::sync::oneshot::Sender<Vec<u8>>,
        secret: Option<Vec<u8>>,
    ) -> Self {
        let oneshot_sender = if let Some(secret) = &secret {
            let _ = oneshot_sender.send(secret.clone());
            None
        } else {
            Some(oneshot_sender)
        };

        Self {
            overlay_broadcast_tx,
            receiver,
            secret,
            oneshot_sender,
        }
    }

    pub async fn handle_messages(mut self) -> anyhow::Result<Self> {
        let overlay_send = self.overlay_broadcast_tx.clone();
        if self.secret.is_none() {
            tokio::time::sleep(Duration::from_secs(2)).await;
            overlay_send
                .send(OverlayMessage::new_p2p_encrypted(
                    None,
                    bincode::serialize(&OverlayMessageType::RequestSharedSecret).unwrap(),
                ))
                .unwrap();
        }

        while let Some(message) = self.receiver.recv().await {
            match message.message {
                MaybeEncrypted::EncryptedP2P(decrypted) => {
                    let overlay_message: OverlayMessageType =
                        bincode::deserialize(&decrypted).unwrap();

                    self.handle_instruction(
                        overlay_message,
                        message.targets.as_ref().unwrap()[0].clone(),
                    )
                    .await?;
                } // NB: full impl has more messages
            }
        }

        Ok(self)
    }

    pub async fn handle_instruction(
        &mut self,
        message: OverlayMessageType,
        _from_peer: Vec<u8>,
    ) -> anyhow::Result<()> {
        match message {
            OverlayMessageType::SharedSecret(NotifySharedSecret { secret }) => {
                tracing::info!(
                    "received shared dstack secret, sending to helios light client task"
                );
                self.secret = Some(secret.clone());
                if let Some(sender) = self.oneshot_sender.take() {
                    if sender.send(secret).is_ok() {
                        tracing::info!("sent secret to light client")
                    } else {
                        tracing::warn!("already sent secret to light client")
                    }
                }
            }
            OverlayMessageType::RequestSharedSecret => {
                tracing::debug!("received request to get dstack secet");
                if let Some(secret) = &self.secret {
                    tracing::debug!("we have shared secret and will share it");
                    let message = bincode::serialize(&OverlayMessageType::SharedSecret(
                        NotifySharedSecret {
                            secret: secret.clone(),
                        },
                    ))?;

                    let _ = self.overlay_broadcast_tx.send(OverlayMessage {
                        targets: None,
                        message: overlay::message::MaybeEncrypted::EncryptedP2P(message),
                    });
                } else {
                    tracing::debug!("we don't have shared secret");
                }
            }
            _ => (),
        }

        Ok(())
    }
}
