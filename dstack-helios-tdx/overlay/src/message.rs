use rand::Rng;
use serde::{Deserialize, Serialize};

pub type Quote = String;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NotifySharedSecret {
    pub secret: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum MaybeEncrypted {
    /// Encrypted to the target pubkey.
    EncryptedP2P(Vec<u8>),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OverlayHeader {
    /// Incremental value of the personal view of the messages interchanged
    /// during the connection.
    pub nonce: i64,
    /// Once mutually attested each party sends their generated
    /// random number and the highest one is chosen as session key.
    /// There's a handful of ways we can deal with this, I'll leave it TBD.
    pub session_key: i64,
    /// Signature of [`pubkey`] for sha256(serialize(header)+serialize(message)).
    pub signature: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OverlayMessage {
    /// Allows the p2p manager to know that we want the message to only be sent if
    /// the connection matches one of the targeted public keys. This is important for communicating
    /// purpusefully with isolated nodes. For example, this is crucial to load-balance the work
    /// to the solvers else we'll be spawning solvency checks on all connected solvers which doesn't
    /// allow us to horizontally scale.
    pub targets: Option<Vec<Vec<u8>>>,
    // tbd: see if needs more fields else collapse on packet.
    pub message: MaybeEncrypted,
}

impl OverlayMessage {
    pub fn new_p2p_encrypted(targets: Option<Vec<Vec<u8>>>, message: Vec<u8>) -> Self {
        Self {
            targets,
            message: MaybeEncrypted::EncryptedP2P(message),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OverlayPacket {
    /// Overlay header. Optional because we establish the header during mutual attestation
    pub header: Option<OverlayHeader>,
    /// TD sending the message
    pub pubkey: Vec<u8>,
    /// Encrypted message. Decryption is left to the application logic
    pub message: OverlayMessage,
}

impl OverlayPacket {
    pub fn from_quote(
        pubkey: &[u8],
        quote: Quote,
        want_shared: bool,
    ) -> anyhow::Result<(Self, i64)> {
        let mut rng = rand::rng();
        let session_key = rng.random();
        let onboard = OverlayOnboard {
            quote,
            session: session_key,
            want_shared,
        };
        let message = OverlayMessage::new_p2p_encrypted(
            None,
            bincode::serialize(&OverlayMessageType::Onboard(onboard)).unwrap(),
        );

        let packet = Self {
            pubkey: pubkey.to_vec(),
            header: None,
            message,
        };

        Ok((packet, session_key))
    }

    pub fn to_payload(&self) -> Option<Vec<u8>> {
        if let Some(header) = &self.header {
            let message_encrypted = match &self.message.message {
                MaybeEncrypted::EncryptedP2P(message) => message.to_vec(),
                // .. full implemenation reserves more messages here.
            };

            Some(
                [
                    self.pubkey.clone(),
                    message_encrypted.to_vec(),
                    header.nonce.to_be_bytes().to_vec(),
                    header.session_key.to_be_bytes().to_vec(),
                ]
                .concat(),
            )
        } else {
            None
        }
    }

    pub fn add_signature(&mut self, signature: Vec<u8>) {
        if let Some(mut header) = self.header.clone() {
            header.signature = signature;
            self.header = Some(header);
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OverlayOnboard {
    pub quote: Quote,
    pub session: i64,
    pub want_shared: bool,
}

// NB: Full impl overlay has many more messages types that can also be passed
// as generics depending on app layer.
#[derive(Serialize, Deserialize, Debug)]
pub enum OverlayMessageType {
    SharedSecret(NotifySharedSecret),
    RequestSharedSecret,
    Onboard(OverlayOnboard),
}
