/// Returns the report data as hex string. Should be replaced with call to the dstack-guest backend to get
/// a quote.
#[cfg(feature = "tdx")]
use tdx_attestation::{Attestation, InnerAttestationHelper};

#[cfg(feature = "tdx")]
pub async fn get_quote(report_data: &[u8]) -> anyhow::Result<String> {
    let attestation = Attestation::new();
    attestation.get_quote(report_data.to_vec()).await
}

#[cfg(not(feature = "tdx"))]
pub async fn get_quote(report_data: &[u8]) -> anyhow::Result<String> {
    Ok(hex::encode(report_data))
}

/// Returns a random secret.
pub fn get_node_secret() -> secp256k1::SecretKey {
    secp256k1::SecretKey::new(&mut secp256k1::rand::thread_rng())
}

// dummy type.
pub struct QuoteVerifyMock {
    pub is_valid: bool,
}

// TODO: actually verify measurements.
#[cfg(feature = "tdx")]
pub async fn verify_quote(quote: &str, appdata: &[u8]) -> QuoteVerifyMock {
    let attestation = Attestation::new();
    let verification = attestation.verify_quote(quote.to_string()).await;

    if verification.is_ok() {
        QuoteVerifyMock {
            is_valid: true,
            is_clearing_house: false,
        }
    } else {
        QuoteVerifyMock {
            is_valid: false,
            is_clearing_house: false,
        }
    }
}

#[cfg(not(feature = "tdx"))]
pub async fn verify_quote(_quote: &str, _appdata: &[u8]) -> QuoteVerifyMock {
    QuoteVerifyMock { is_valid: true }
}

/// Should return information about the virtal tsc.
pub fn get_tsc(infer: u64) -> u64 {
    infer
}

#[cfg(test)]
mod test {
    use sha2::{Digest, Sha256};
    use tdx_attestation::{Attestation, InnerAttestationHelper};

    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct AttestResponse {
        quote: String,
        pubkey: String,
    }

    fn calc_report_data(appdata: &[u8]) -> [u8; 64] {
        let preimage = format!("register{}", hex::encode(appdata));
        let mut hasher = Sha256::new();
        hasher.update(preimage);
        let hashed: Vec<u8> = hasher.finalize().to_vec();
        let mut padded_report_data = [0_u8; 64];
        padded_report_data[..hashed.len()].copy_from_slice(&hashed);

        padded_report_data
    }

    #[tokio::test]
    async fn test_verify_quote() {
        let url = "http://34.19.110.223:3032/attest";

        let response = reqwest::get(url).await.unwrap().bytes().await.unwrap();
        let response = serde_json::from_str::<AttestResponse>(
            &serde_json::from_slice::<String>(&response).unwrap(),
        )
        .unwrap();
        let pubkey = hex::decode(response.pubkey).unwrap();

        let attestation = Attestation::new();
        let verification = attestation.verify_quote(response.quote.to_string()).await;
        assert!(verification.is_ok());
        assert_eq!(
            verification.unwrap().report.as_td10().unwrap().report_data,
            calc_report_data(&pubkey)
        );
    }
}
