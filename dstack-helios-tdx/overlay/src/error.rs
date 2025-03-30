use thiserror::Error;

#[derive(Error, Debug)]
pub enum OverlayError {
    #[error("Expected quote as first message but received another message type.")]
    GotNoQuote,

    //#[error("Expected to onboard due to absent local view, but got full message with header")]
    //MalformedOnboard,
    #[error("Got invalid quote {0}")]
    InvaildQuote(crate::message::Quote),

    #[error("Invalid session key. Have {0}, got {1}")]
    InvalidSessionKey(i64, i64),

    #[error("Invalid session nonce. Have {0}, got {1}")]
    InvalidNonce(i64, i64),

    // NB: this is an indicator of host interference.
    #[error("Used an expired nonce {0}")]
    ExpiredNonce(i64),
}
