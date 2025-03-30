use aes_gcm::{
    aead::{generic_array::GenericArray, Aead},
    aes::Aes256,
    Aes256Gcm, AesGcm, KeyInit,
};
use anyhow::anyhow;
use secp256k1::{ecdh::SharedSecret, PublicKey, SecretKey};
use sha2::digest::consts::U12;

pub struct ChiperWrapper {
    inner: AesGcm<Aes256, U12>,
}

impl ChiperWrapper {
    pub fn new(local_secret: &[u8; 32], peer_pubkey: &[u8]) -> anyhow::Result<Self> {
        let pubkey = PublicKey::from_slice(peer_pubkey)?;
        let secret = SecretKey::from_byte_array(local_secret)?;
        let p2p_secret = SharedSecret::new(&pubkey, &secret).secret_bytes();
        let chiper = {
            let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&p2p_secret);
            Aes256Gcm::new(key)
        };

        Ok(Self { inner: chiper })
    }
    /// Decrypts a message given the shared secret.
    pub fn get_decrypted_message(&self, encrypted_message: &[u8]) -> anyhow::Result<Vec<u8>> {
        let decrypted = self
            .inner
            .decrypt(GenericArray::from_slice(&[0; 12]), encrypted_message)
            .map_err(|e| anyhow!(e))?;

        Ok(decrypted)
    }

    /// Encrypts a message given the [`shared_secret`].
    pub fn get_encrypted_message(&self, plain_message: &[u8]) -> anyhow::Result<Vec<u8>> {
        let encrypted = self
            .inner
            .encrypt(GenericArray::from_slice(&[0; 12]), plain_message)
            .map_err(|e| anyhow!(e))?;

        Ok(encrypted)
    }
}
