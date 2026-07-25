//! CAdES-B-B detached signing for arbitrary files.
//!
//! Produces a detached CMS `SignedData` (`.p7s`) over the SHA-256 of the input
//! bytes, using the same on-card signing path as PAdES.

use crate::cie::cms;
use crate::cie::error::CieResult;
use crate::cie::pkcs11::CardSigner;

/// Sign `data` and return the detached CAdES-B-B signature (DER-encoded CMS).
pub fn sign_detached(
    data: &[u8],
    module_path: &str,
    slot_id: u64,
    cert_id_hex: &str,
    pin: &str,
) -> CieResult<Vec<u8>> {
    let signer = CardSigner::open(module_path, slot_id, cert_id_hex, pin)?;

    let digest = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(data);
        let out: [u8; 32] = h.finalize().into();
        out
    };

    cms::build_detached_pkcs7(signer.certificate_der(), &digest, |tbs| signer.sign(tbs))
}
