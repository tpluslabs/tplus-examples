//! Flow is really simple here.
//! 
//! We don't need replication so when the guest spins up it generates a random associated key. 
//! When the replication thread starts it generates a quote using the associated key's pubkey as report
//! data and then forwards it to the host's bootstrap endpoint that can publish the quote however it wants.
//! 
//! The only tdx guest path we want to expose now is the associated key's one since the helios light client
//! will need it to sign over the messages. Anyone can verify the quote for the public key singning the message.
//! 
use async_trait::async_trait;
use dstack_core::{
    host_paths, GuestServiceInner, HostServiceInner, InnerAttestationHelper, TdxOnlyGuestServiceInner
};
use tdx_attestation::Attestation;
// TODO change types depending on the chain we're posting to.
pub struct HostServices;

// Since we're not replicating we don't need to register or onboard. but we can
// kinda bootstrap by just posting the quote somewhere for discovery.
#[async_trait]
impl HostServiceInner for HostServices {
    type Pubkey = ();
    type Quote = String;
    type Signature = ();

    async fn bootstrap(
        &self,
        quote: Self::Quote,
        _pubkeys: Vec<Self::Pubkey>,
    ) -> anyhow::Result<()> {
        // todo post quote somewhere.
        println!("Got quote");
        let decoded = hex::decode(quote)?;
        let parsed = dcap_qvl::quote::Quote::parse(&decoded)?;

        let td_report = parsed.report.as_td10().ok_or::<anyhow::Error>(anyhow::anyhow!("invalid quote type").into())?;
        println!("Static measurements:");
        println!("mrtd: {}", hex::encode(td_report.mr_td));
        
        println!("Runtime measurements:");
        println!("0: {}", hex::encode(td_report.rt_mr0));
        println!("1: {}", hex::encode(td_report.rt_mr1));
        println!("2: {}", hex::encode(td_report.rt_mr2));
        println!("3: {}", hex::encode(td_report.rt_mr3));
        Ok(())
    }

    async fn register(
        &self,
        _quote: Self::Quote,
        _pubkeys: Vec<Self::Pubkey>,
        _signatures: Vec<Self::Signature>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn onboard_thread(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct GuestServices {
    // Implementor's configs including helper objects.
    host_endpoint: String,
    // kettle's associated key.
    associated_key: [u8; 32],
    attestation: Attestation,
}

impl GuestServices {
    pub fn new() -> Self {
        //let host_address = std::env::var("HOST").unwrap_or("host.containers.internal:8000".into());
        let host_address = std::env::var("HOST").unwrap_or("127.0.0.1:8000".into());
        let associated_key = secp256k1::SecretKey::new(&mut secp256k1::rand::thread_rng()).secret_bytes();

        Self {
            host_endpoint: host_address,
            associated_key,
            attestation: Attestation::new(),
        }
    }
}

// We don't need any of the dstack replication functionalities.
#[async_trait]
impl GuestServiceInner for GuestServices {
    type Pubkey = ();
    type EncryptedMessage = ();
    type SharedKey = ();
    type Quote = String;

    // Note: the implementor decides for themselves how they want the secret to be stored in
    // [`self`]
    async fn get_secret(&self) -> anyhow::Result<Self::SharedKey> {
        Ok(())
    }

    async fn replicate_thread(&self) -> anyhow::Result<()> {
        let secp = secp256k1::Secp256k1::new();
        let associated = secp256k1::SecretKey::from_byte_array(&self.associated_key)?;
        
        println!("getting quote");
        //let quote = "realquotewillbehere".to_string();
        let quote = self.attestation.get_quote(associated.public_key(&secp).serialize().to_vec()).await?;
        println!("got quote");
        
        let client = reqwest::Client::new();

        client
            .post(format!("http://{}/bootstrap", self.host_endpoint))
            .json(&host_paths::requests::BootstrapArgs::<HostServices> {
                quote,
                pubkeys: vec![],
            })
            .send()
            .await?
            .text()
            .await?;
        println!("Requested bootstrap");
        
        Ok(())
    }

    /// Verifies the provided quote ensuring that [`pubkeys[0]`] is within the quote, if that
    /// succeeds (i.e secretkey is held only in tdx) then it encrypts the shared secret to
    /// [`pubkeys[0]`].
    async fn onboard_new_node(
        &self,
        _quote: Self::Quote,
        _pubkeys: Vec<Self::Pubkey>,
    ) -> anyhow::Result<Self::EncryptedMessage> {
        Ok(())
    }
}

/// NON host-facing paths here.
#[async_trait]
impl TdxOnlyGuestServiceInner for GuestServices {
    type Tag = ();
    type DerivedKey = ();
    type AssociatedKey = String;

    // there is no shared secret, so no derived key.
    async fn get_derived_key(&self, _tag: Self::Tag) -> anyhow::Result<Self::DerivedKey> {
        Ok(())
    }

    async fn get_associated_key(&self) -> anyhow::Result<Self::AssociatedKey> {
        Ok(hex::encode(&self.associated_key))
    }
}
