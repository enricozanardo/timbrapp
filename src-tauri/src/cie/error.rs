//! Error type shared across the CIE signing backend.
//!
//! Everything funnels into [`CieError`], which serializes to a plain string so
//! it can cross the Tauri IPC boundary and surface directly in the UI.

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum CieError {
    #[error("PKCS#11 module not found at '{0}'. Install the official Software CIE (or an opencie/cie-middleware build) and point the app at its PKCS#11 library.")]
    ModuleNotFound(String),

    #[error("PKCS#11 error: {0}")]
    Pkcs11(#[from] cryptoki::error::Error),

    #[error("Slot {0} not found. Re-scan readers and try again.")]
    SlotNotFound(u64),

    #[error("No signing certificate found on the card (looked for id '{0}').")]
    CertificateNotFound(String),

    #[error("No private key on the card matches the selected certificate.")]
    PrivateKeyNotFound,

    #[error("Wrong PIN or the card is locked. Check the CIE PIN (3 wrong attempts block the card; the PUK is required to unblock).")]
    PinIncorrect,

    #[error("Unsupported key type on the card: {0}. Only RSA CIE keys are supported for now.")]
    UnsupportedKeyType(String),

    #[error("PDF error: {0}")]
    Pdf(String),

    #[error("The signature placeholder is too small for the produced signature ({needed} bytes needed, {available} available). Increase the reserved size.")]
    PlaceholderTooSmall { needed: usize, available: usize },

    #[error("ASN.1/DER encoding error: {0}")]
    Der(String),

    #[error("{0}")]
    Other(String),
}

impl From<der::Error> for CieError {
    fn from(e: der::Error) -> Self {
        CieError::Der(e.to_string())
    }
}

impl From<lopdf::Error> for CieError {
    fn from(e: lopdf::Error) -> Self {
        CieError::Pdf(e.to_string())
    }
}

impl Serialize for CieError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type CieResult<T> = Result<T, CieError>;
