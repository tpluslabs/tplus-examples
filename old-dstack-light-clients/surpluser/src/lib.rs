use anyhow::{anyhow, Result};
use helios::ethereum::{
    config::networks::Network, database::FileDB, EthereumClient, EthereumClientBuilder,
};
use secp256k1::{Message, Secp256k1, SecretKey};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use warp::Filter;

async fn get_associated_key() -> Result<[u8; 32]> {
    let resp: Value = reqwest::get("http://localhost:3031/getnodekey").await?.json().await?;
    let decoded = hex::decode(&resp.as_str().unwrap())?;
    Ok(decoded.try_into().unwrap())
}

pub async fn run() -> Result<()> {
    println!("starting surpluser");
    let secp = Secp256k1::new();
    println!("getting associated key");
    let node_secret_key = get_associated_key().await?;
    println!("got associated key");
    let secret_key = secp256k1::SecretKey::from_byte_array(&node_secret_key)?;
    println!("node pubkey {}", secret_key.public_key(&secp));

    let untrusted_rpc_url = "https://eth-mainnet.g.alchemy.com/v2/demo";
    println!("Using untrusted RPC URL {}", untrusted_rpc_url);

    let consensus_rpc = "https://www.lightclientdata.org";
    println!("Using consensus RPC URL: {}", consensus_rpc);

    let mut client: EthereumClient<FileDB> = EthereumClientBuilder::new()
        .network(Network::Mainnet)
        .consensus_rpc(consensus_rpc)
        .execution_rpc(untrusted_rpc_url)
        // we should turn this off in prod and find a good way to retrieve a trusted checkpoint
        .load_external_fallback()
        .data_dir(PathBuf::from("/tmp/helios"))
        .build()
        .map_err(|e| anyhow!(e.to_string()))?;

    println!(
        "Built client on network \"{}\" with external checkpoint fallbacks",
        Network::Mainnet
    );

    client.start().await.map_err(|e| anyhow!(e.to_string()))?;
    client.wait_synced().await;
    println!("client synced");

    let client = Arc::new(client);
    let get_trusted_block = warp::path("block").and_then({
        let client = client.clone();
        let node_secret_key = node_secret_key;
        move || {
            let client = client.clone();
            async move {
                let block = client.get_block_number().await.unwrap().to_string();
                let signature = sign_message(node_secret_key, &block);
                Ok::<_, warp::Rejection>(warp::reply::json(
                    &serde_json::json!({"signature": hex::encode(&signature), "blocknum": block})
                        .to_string(),
                ))
            }
        }
    });

    warp::serve(get_trusted_block)
        .run(([0, 0, 0, 0], 3032))
        .await;

    Ok(())
}

fn sign_message(secret: [u8; 32], message: &str) -> [u8; 64] {
    let mut hasher = Sha256::new();
    hasher.update(message);
    let msg = Message::from_digest(hasher.finalize().into());
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&secret).unwrap();
    let signature = secp.sign_ecdsa(&msg, &secret_key);
    signature.serialize_compact()
}

#[cfg(test)]
mod test {
    use secp256k1::{ecdsa::Signature, PublicKey};

    use super::*;

    #[test]
    fn verify_message() {
        let message_raw = "test";
        let secp = Secp256k1::new();
        let secret = [1_u8; 32];
        let secret_key = SecretKey::from_slice(&secret).unwrap();
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);

        let signature = sign_message(secret, &message_raw);
        let signature = Signature::from_compact(&signature).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(message_raw);
        let msg: Message = Message::from_digest(hasher.finalize().into());
        assert!(secp.verify_ecdsa(&msg, &signature, &public_key).is_ok());
    }

    #[test]
    fn test() {
        let secp = Secp256k1::new();

        let pubkey =
            hex::decode("028e8e7d5a8164b7495835a1d8fd3ef0885b12cd13e02c3e98291e1e74c8b1dec1")
                .unwrap();
        let public_key = PublicKey::from_slice(&pubkey).unwrap();
        let message_raw = "21855128";

        let signature = Signature::from_compact(&hex::decode("b78d9d9e0aa5aef1af6a5942e40bdd97ffe55d1798a21fd8b99552d9bc74ba9f3fde648b98fd53efb208b7eb5db9b5321743c02a0703e9b954cd6d9a99302b9d").unwrap()).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(message_raw);
        let msg: Message = Message::from_digest(hasher.finalize().into());
        assert!(secp.verify_ecdsa(&msg, &signature, &public_key).is_ok());
    }
}
