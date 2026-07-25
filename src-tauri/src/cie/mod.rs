//! CIE (Carta d'Identita Elettronica) local signing backend.
//!
//! Implements the official CieSign "Desktop" model (Firma Elettronica Avanzata,
//! eIDAS art. 26 - no QTSP / no cloud): a contactless/NFC PC-SC reader talks to
//! the CIE chip through the official CIE PKCS#11 middleware, and this backend
//! assembles PAdES/CAdES signatures around the on-card signature.
//!
//! All heavy lifting (readers, certificates, signing) is exposed to the Svelte
//! frontend as Tauri commands defined at the bottom of this file.

mod cades;
mod cms;
pub mod enroll;
mod error;
pub mod pades;
pub mod pkcs11;

use base64::Engine;
use serde::Serialize;

use enroll::EnrollOutcome;
use error::{CieError, CieResult};

/// A PC-SC slot/reader that currently has a card inserted.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderInfo {
    pub slot_id: u64,
    pub slot_description: String,
    pub manufacturer: String,
    pub token_present: bool,
    pub token_label: Option<String>,
}

/// A certificate available on the card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificateInfo {
    /// PKCS#11 CKA_ID, hex-encoded. Used to select the key/cert for signing.
    pub id_hex: String,
    pub label: String,
    pub subject: String,
    pub issuer: String,
    pub serial_hex: String,
    pub not_before: String,
    pub not_after: String,
    pub slot_id: u64,
    /// True when KeyUsage marks this cert for signing (digitalSignature or
    /// nonRepudiation) - the likely FEA/subscription certificate.
    pub key_usage_sign: bool,
}

/// Diagnostics summary for the "is my setup ready to sign?" panel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CieStatus {
    /// Resolved PKCS#11 module path, if one was found.
    pub module_path: Option<String>,
    pub module_found: bool,
    pub readers: Vec<ReaderInfo>,
    /// Human-readable hint when something is missing.
    pub message: String,
}

/// Candidate paths for a CIE PKCS#11 module shipped *inside* TimbrApp
/// (under `resources/pkcs11/`). When present, this lets users install only
/// TimbrApp - no separate "Software CIE" install needed.
fn bundled_module_candidates(app: &tauri::AppHandle) -> Vec<String> {
    use tauri::Manager;
    let Ok(dir) = app.path().resource_dir() else {
        return vec![];
    };
    let base = dir.join("pkcs11");
    let names: &[&str] = if cfg!(target_os = "windows") {
        &["cie-pkcs11.dll", "libcie-pkcs11.dll", "bit4p11.dll"]
    } else if cfg!(target_os = "macos") {
        &["libcie-pkcs11.dylib"]
    } else {
        &["libcie-pkcs11.so"]
    };
    names
        .iter()
        .map(|n| base.join(n).to_string_lossy().into_owned())
        .collect()
}

fn b64_decode(s: &str) -> CieResult<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| CieError::Other(format!("invalid base64 input: {e}")))
}

fn b64_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Probe the environment: locate the PKCS#11 module and list readers with a card.
#[tauri::command]
pub fn cie_status(app: tauri::AppHandle, module_path: Option<String>) -> CieStatus {
    let bundled = bundled_module_candidates(&app);
    let resolved = pkcs11::resolve_module_path(module_path.as_deref(), &bundled);
    match resolved {
        Ok(path) => {
            match pkcs11::list_readers(&path) {
                Ok(readers) if !readers.is_empty() => CieStatus {
                    module_path: Some(path),
                    module_found: true,
                    message: format!("{} reader(s) with a card detected.", readers.len()),
                    readers,
                },
                Ok(_) => CieStatus {
                    module_path: Some(path),
                    module_found: true,
                    readers: vec![],
                    message: "PKCS#11 module loaded, but no reader has a CIE inserted. Place the card on the NFC reader.".into(),
                },
                Err(e) => CieStatus {
                    module_path: Some(path),
                    module_found: true,
                    readers: vec![],
                    message: format!("PKCS#11 module loaded but reader scan failed: {e}"),
                },
            }
        }
        Err(e) => CieStatus {
            module_path: None,
            module_found: false,
            readers: vec![],
            message: e.to_string(),
        },
    }
}

/// List readers (slots with a token present).
#[tauri::command]
pub fn cie_list_readers(
    app: tauri::AppHandle,
    module_path: Option<String>,
) -> CieResult<Vec<ReaderInfo>> {
    let bundled = bundled_module_candidates(&app);
    let path = pkcs11::resolve_module_path(module_path.as_deref(), &bundled)?;
    pkcs11::list_readers(&path)
}

/// List certificates on the card in the given slot.
#[tauri::command]
pub fn cie_list_certificates(
    app: tauri::AppHandle,
    module_path: Option<String>,
    slot_id: u64,
) -> CieResult<Vec<CertificateInfo>> {
    let bundled = bundled_module_candidates(&app);
    let path = pkcs11::resolve_module_path(module_path.as_deref(), &bundled)?;
    pkcs11::list_certificates(&path, slot_id)
}

/// Raw on-card signature proof-of-concept: SHA256+RSA sign arbitrary bytes.
/// Returns the signature (base64). Used to validate the key + PIN end-to-end
/// before wiring up full PAdES.
#[tauri::command]
pub fn cie_sign_raw(
    app: tauri::AppHandle,
    module_path: Option<String>,
    slot_id: u64,
    cert_id_hex: String,
    pin: String,
    data_base64: String,
) -> CieResult<String> {
    let bundled = bundled_module_candidates(&app);
    let path = pkcs11::resolve_module_path(module_path.as_deref(), &bundled)?;
    let data = b64_decode(&data_base64)?;
    let signer = pkcs11::CardSigner::open(&path, slot_id, &cert_id_hex, &pin)?;
    let sig = signer.sign(&data)?;
    Ok(b64_encode(&sig))
}

/// Sign a PDF (base64 in / base64 out) as PAdES-B-B.
#[tauri::command]
pub fn cie_sign_pdf(
    app: tauri::AppHandle,
    module_path: Option<String>,
    slot_id: u64,
    cert_id_hex: String,
    pin: String,
    pdf_base64: String,
    reason: Option<String>,
    location: Option<String>,
) -> CieResult<String> {
    let bundled = bundled_module_candidates(&app);
    let path = pkcs11::resolve_module_path(module_path.as_deref(), &bundled)?;
    let pdf = b64_decode(&pdf_base64)?;
    let signed = pades::sign_pdf(
        &pdf,
        &path,
        slot_id,
        &cert_id_hex,
        &pin,
        reason.as_deref(),
        location.as_deref(),
    )?;
    Ok(b64_encode(&signed))
}

/// Check whether the CIE in the given slot is already enrolled (paired).
///
/// PIN-free: safe to call before deciding whether to prompt for enrollment.
#[tauri::command]
pub fn cie_is_enrolled(
    app: tauri::AppHandle,
    module_path: Option<String>,
    slot_id: u64,
) -> CieResult<bool> {
    let bundled = bundled_module_candidates(&app);
    let path = pkcs11::resolve_module_path(module_path.as_deref(), &bundled)?;
    pkcs11::is_enrolled(&path, slot_id)
}

/// One-time CIE enrollment ("Abilita CIE"). Requires the full 8-digit PIN.
///
/// SAFETY: this is a single, no-retry call to the vendor `AbilitaCIE`. A wrong
/// PIN consumes one of the card's 3 attempts, so the caller must confirm the
/// card is not already enrolled (via [`cie_is_enrolled`]) and act only on
/// explicit user intent.
#[tauri::command]
pub fn cie_enroll(
    app: tauri::AppHandle,
    module_path: Option<String>,
    pin: String,
) -> CieResult<EnrollOutcome> {
    let bundled = bundled_module_candidates(&app);
    let path = pkcs11::resolve_module_path(module_path.as_deref(), &bundled)?;
    enroll::abilita_cie(&path, &pin)
}

/// Sign arbitrary bytes (base64 in) as a detached CAdES-B-B `.p7s` (base64 out).
#[tauri::command]
pub fn cie_sign_bytes(
    app: tauri::AppHandle,
    module_path: Option<String>,
    slot_id: u64,
    cert_id_hex: String,
    pin: String,
    data_base64: String,
) -> CieResult<String> {
    let bundled = bundled_module_candidates(&app);
    let path = pkcs11::resolve_module_path(module_path.as_deref(), &bundled)?;
    let data = b64_decode(&data_base64)?;
    let p7s = cades::sign_detached(&data, &path, slot_id, &cert_id_hex, &pin)?;
    Ok(b64_encode(&p7s))
}
