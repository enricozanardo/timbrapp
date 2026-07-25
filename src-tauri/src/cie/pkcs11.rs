//! Thin wrapper over a CIE PKCS#11 module (loaded at runtime via `cryptoki`).
//!
//! We deliberately talk to the card *only* through a PKCS#11 module (the one
//! shipped by the official "Software CIE" or an `opencie`/`cie-middleware`
//! build). That module already implements the delicate CIE secure-messaging /
//! APDU layer, so we never reimplement it. Reader detection is done by listing
//! PKCS#11 slots, which avoids a build-time dependency on `libpcsclite` (handy
//! for developing inside WSL where PC/SC is unavailable).

use cryptoki::context::{CInitializeArgs, Pkcs11};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, KeyType, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};
use cryptoki::slot::Slot;
use cryptoki::types::AuthPin;
use std::path::Path;

use super::{CertificateInfo, ReaderInfo};
use crate::cie::error::{CieError, CieResult};

/// Candidate default locations for the CIE PKCS#11 module, tried in order when
/// the caller does not pass an explicit path. These are best-effort guesses;
/// the UI always lets the user override with an exact path.
pub fn default_module_candidates() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec![
            // Official IPZS "Middleware CIE" installs its PKCS#11 module here.
            r"C:\Windows\System32\CIEPKI.dll".into(),
            r"C:\Windows\SysWOW64\CIEPKI.dll".into(),
            r"C:\Windows\System32\cie-pkcs11.dll".into(),
            r"C:\Windows\System32\bit4p11.dll".into(),
            r"C:\Program Files\Software CIE\cie-pkcs11.dll".into(),
        ]
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            "/Library/CIE/libcie-pkcs11.dylib".into(),
            "/usr/local/lib/libcie-pkcs11.dylib".into(),
            "/Applications/Software CIE.app/Contents/Frameworks/libcie-pkcs11.dylib".into(),
        ]
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        vec![
            "/usr/lib/libcie-pkcs11.so".into(),
            "/usr/lib/x86_64-linux-gnu/libcie-pkcs11.so".into(),
            "/usr/local/lib/libcie-pkcs11.so".into(),
            "/opt/CIE/libcie-pkcs11.so".into(),
        ]
    }
}

/// Resolve the module path in priority order:
/// 1. an explicit path from the caller (errors if it does not exist);
/// 2. a module bundled inside the app (`bundled` candidates) - this is what lets
///    users install *only* TimbrApp;
/// 3. a system-installed module (official "Software CIE" default locations).
pub fn resolve_module_path(explicit: Option<&str>, bundled: &[String]) -> CieResult<String> {
    if let Some(p) = explicit {
        if Path::new(p).exists() {
            return Ok(p.to_string());
        }
        return Err(CieError::ModuleNotFound(p.to_string()));
    }
    for cand in bundled.iter().cloned().chain(default_module_candidates()) {
        if Path::new(&cand).exists() {
            return Ok(cand);
        }
    }
    Err(CieError::ModuleNotFound(
        "<no bundled or system CIE PKCS#11 module found>".into(),
    ))
}

/// Run a closure that calls into the (native, third-party) PKCS#11 module,
/// converting a Rust panic into a clean error instead of aborting the whole
/// app. This does not protect against a hard native crash inside the module,
/// but it does contain panics originating in the binding layer.
fn guard_ffi<T>(what: &str, f: impl FnOnce() -> CieResult<T>) -> CieResult<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(res) => res,
        Err(_) => {
            log::error!("panic while {what}");
            Err(CieError::Other(format!(
                "the PKCS#11 module crashed while {what}. It may be incompatible \
                 or require its middleware/service to be running."
            )))
        }
    }
}

/// Load + initialize the PKCS#11 module.
fn open_module(module_path: &str) -> CieResult<Pkcs11> {
    if !Path::new(module_path).exists() {
        return Err(CieError::ModuleNotFound(module_path.to_string()));
    }
    log::info!("loading PKCS#11 module: {module_path}");
    let pkcs11 = guard_ffi("loading the PKCS#11 module", || Ok(Pkcs11::new(module_path)?))?;
    log::info!("initializing PKCS#11 module");
    guard_ffi("initializing the PKCS#11 module", || {
        // Some modules report "already initialized" if a previous instance did
        // not finalize cleanly; treat that as success rather than failing.
        match pkcs11.initialize(CInitializeArgs::OsThreads) {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().contains("already been initialized") => {
                log::warn!("module was already initialized; reusing");
                Ok(())
            }
            Err(e) => Err(CieError::from(e)),
        }
    })?;
    log::info!("PKCS#11 module ready");
    Ok(pkcs11)
}

fn find_slot(pkcs11: &Pkcs11, slot_id: u64) -> CieResult<Slot> {
    let slots = pkcs11.get_slots_with_token()?;
    slots
        .into_iter()
        .find(|s| s.id() == slot_id)
        .ok_or(CieError::SlotNotFound(slot_id))
}

/// List readers/slots that currently have a token (card) present.
pub fn list_readers(module_path: &str) -> CieResult<Vec<ReaderInfo>> {
    let pkcs11 = open_module(module_path)?;
    let slots = pkcs11.get_slots_with_token()?;
    let mut out = Vec::new();
    for slot in slots {
        let slot_info = pkcs11.get_slot_info(slot)?;
        let token_info = pkcs11.get_token_info(slot).ok();
        out.push(ReaderInfo {
            slot_id: slot.id(),
            slot_description: slot_info.slot_description().trim().to_string(),
            manufacturer: slot_info.manufacturer_id().trim().to_string(),
            token_present: true,
            token_label: token_info.map(|t| t.label().trim().to_string()),
        });
    }
    Ok(out)
}

/// PIN-free "is this card already enrolled?" probe.
///
/// On the CIE contactless interface an *un-enrolled* card cannot be read: the
/// module opens the session but fails to parse card data ("BER decode error").
/// So we treat "session opens AND at least one certificate is readable" as
/// enrolled. This needs no PIN, so it can be called freely before deciding
/// whether to prompt for enrollment.
pub fn is_enrolled(module_path: &str, slot_id: u64) -> CieResult<bool> {
    let pkcs11 = open_module(module_path)?;
    let slot = find_slot(&pkcs11, slot_id)?;
    match pkcs11.open_ro_session(slot) {
        Ok(session) => match session.find_objects(&[Attribute::Class(ObjectClass::CERTIFICATE)]) {
            Ok(handles) => Ok(!handles.is_empty()),
            // Session opened but card data can't be read yet -> not enrolled.
            Err(_) => Ok(false),
        },
        // Opening the session itself fails on an un-enrolled contactless CIE.
        Err(_) => Ok(false),
    }
}

fn attr_bytes(attrs: &[Attribute], want: AttributeType) -> Option<Vec<u8>> {
    for a in attrs {
        match (want, a) {
            (AttributeType::Value, Attribute::Value(v)) => return Some(v.clone()),
            (AttributeType::Label, Attribute::Label(v)) => return Some(v.clone()),
            (AttributeType::Id, Attribute::Id(v)) => return Some(v.clone()),
            _ => {}
        }
    }
    None
}

/// Enumerate signing certificates present on the card in the given slot.
pub fn list_certificates(module_path: &str, slot_id: u64) -> CieResult<Vec<CertificateInfo>> {
    let pkcs11 = open_module(module_path)?;
    let slot = find_slot(&pkcs11, slot_id)?;
    let session = pkcs11.open_ro_session(slot)?;

    let handles = session.find_objects(&[Attribute::Class(ObjectClass::CERTIFICATE)])?;
    let mut out = Vec::new();
    for handle in handles {
        let attrs = session.get_attributes(
            handle,
            &[AttributeType::Value, AttributeType::Label, AttributeType::Id],
        )?;
        let der = match attr_bytes(&attrs, AttributeType::Value) {
            Some(v) => v,
            None => continue,
        };
        let id = attr_bytes(&attrs, AttributeType::Id).unwrap_or_default();
        let label = attr_bytes(&attrs, AttributeType::Label)
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default();

        let info = describe_certificate(&der, &id, &label, slot_id)
            .unwrap_or_else(|_| CertificateInfo {
                id_hex: hex::encode(&id),
                label: label.clone(),
                subject: "<unparseable certificate>".into(),
                issuer: String::new(),
                serial_hex: String::new(),
                not_before: String::new(),
                not_after: String::new(),
                slot_id,
                key_usage_sign: false,
            });
        out.push(info);
    }
    // Put likely FEA/signature certs (nonRepudiation) first.
    out.sort_by(|a, b| b.key_usage_sign.cmp(&a.key_usage_sign));
    Ok(out)
}

fn describe_certificate(
    der: &[u8],
    id: &[u8],
    label: &str,
    slot_id: u64,
) -> CieResult<CertificateInfo> {
    use der::Decode;
    let cert = x509_cert::Certificate::from_der(der)?;
    let tbs = &cert.tbs_certificate;

    let key_usage_sign = key_usage_is_signing(tbs);

    Ok(CertificateInfo {
        id_hex: hex::encode(id),
        label: label.to_string(),
        subject: tbs.subject.to_string(),
        issuer: tbs.issuer.to_string(),
        serial_hex: hex::encode(tbs.serial_number.as_bytes()),
        not_before: tbs.validity.not_before.to_string(),
        not_after: tbs.validity.not_after.to_string(),
        slot_id,
        key_usage_sign,
    })
}

/// Return true when the certificate's KeyUsage marks it for signing
/// (digitalSignature or nonRepudiation/contentCommitment). Defaults to true
/// when no KeyUsage extension is present, so a valid cert is never hidden.
fn key_usage_is_signing(tbs: &x509_cert::TbsCertificate) -> bool {
    use der::Decode;
    let Some(exts) = &tbs.extensions else {
        return true;
    };
    for ext in exts.iter() {
        if ext.extn_id == const_oid::db::rfc5280::ID_CE_KEY_USAGE {
            if let Ok(ku) = x509_cert::ext::pkix::KeyUsage::from_der(ext.extn_value.as_bytes()) {
                return ku.digital_signature() || ku.non_repudiation();
            }
        }
    }
    true
}

/// A logged-in signing session bound to one CIE key + certificate.
///
/// Holds the initialized module alive alongside the open session so it can be
/// reused for the (single) on-card signature during a PAdES/CAdES flow.
pub struct CardSigner {
    // Field order matters: `session` must drop before `_pkcs11` finalizes.
    session: Session,
    key: ObjectHandle,
    cert_der: Vec<u8>,
    _pkcs11: Pkcs11,
}

impl CardSigner {
    /// Open a session, verify the PIN, and locate the signer cert + private key
    /// identified by `cert_id_hex`. Only RSA keys are accepted.
    pub fn open(
        module_path: &str,
        slot_id: u64,
        cert_id_hex: &str,
        pin: &str,
    ) -> CieResult<Self> {
        let want_id = hex::decode(cert_id_hex)
            .map_err(|_| CieError::CertificateNotFound(cert_id_hex.to_string()))?;

        let pkcs11 = open_module(module_path)?;
        let slot = find_slot(&pkcs11, slot_id)?;
        let session = pkcs11.open_ro_session(slot)?;

        // PIN login. Map common "wrong PIN / locked" errors to a friendly message.
        if let Err(e) = session.login(UserType::User, Some(&AuthPin::new(pin.to_string()))) {
            if let cryptoki::error::Error::Pkcs11(rv, _) = &e {
                use cryptoki::error::RvError;
                if matches!(
                    rv,
                    RvError::PinIncorrect | RvError::PinLocked | RvError::PinInvalid
                ) {
                    return Err(CieError::PinIncorrect);
                }
            }
            return Err(CieError::Pkcs11(e));
        }

        let cert_der = find_certificate_der(&session, &want_id)?;
        let key = find_private_key(&session, &want_id)?;

        // Only RSA CIE keys are supported for now.
        let key_type = session.get_attributes(key, &[AttributeType::KeyType])?;
        if let Some(Attribute::KeyType(kt)) = key_type.into_iter().next() {
            if kt != KeyType::RSA {
                return Err(CieError::UnsupportedKeyType(format!("{kt:?}")));
            }
        }

        Ok(Self {
            session,
            key,
            cert_der,
            _pkcs11: pkcs11,
        })
    }

    /// DER of the signer certificate.
    pub fn certificate_der(&self) -> &[u8] {
        &self.cert_der
    }

    /// Sign `data` on the card, producing a PKCS#1 v1.5 signature over the
    /// SHA-256 of `data`. For CMS, `data` is the DER of the signed attributes.
    ///
    /// We deliberately hash in software and sign a hand-built SHA-256
    /// `DigestInfo` with raw `CKM_RSA_PKCS`, instead of the card's
    /// `CKM_SHA256_RSA_PKCS`: the IPZS CIE module's combined mechanism is
    /// broken - it emits a malformed **SHA-1** `DigestInfo` (the SHA-256 digest
    /// truncated to 20 bytes), which every verifier rejects as corrupted.
    /// Raw `CKM_RSA_PKCS` (block-type-1 padding done by the module, RSA on the
    /// card) yields a correct, verifiable SHA-256 signature.
    pub fn sign(&self, data: &[u8]) -> CieResult<Vec<u8>> {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(data);
        // DER of DigestInfo { AlgorithmIdentifier { sha256, NULL }, OCTET STRING(hash) }.
        let mut digest_info: Vec<u8> = vec![
            0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
            0x01, 0x05, 0x00, 0x04, 0x20,
        ];
        digest_info.extend_from_slice(&hash);
        Ok(self
            .session
            .sign(&Mechanism::RsaPkcs, self.key, &digest_info)?)
    }
}

impl Drop for CardSigner {
    fn drop(&mut self) {
        let _ = self.session.logout();
    }
}

fn find_certificate_der(session: &Session, want_id: &[u8]) -> CieResult<Vec<u8>> {
    let handles = session.find_objects(&[
        Attribute::Class(ObjectClass::CERTIFICATE),
        Attribute::Id(want_id.to_vec()),
    ])?;
    for handle in handles {
        let attrs = session.get_attributes(handle, &[AttributeType::Value])?;
        if let Some(der) = attr_bytes(&attrs, AttributeType::Value) {
            return Ok(der);
        }
    }
    Err(CieError::CertificateNotFound(hex::encode(want_id)))
}

fn find_private_key(session: &Session, want_id: &[u8]) -> CieResult<ObjectHandle> {
    let handles = session.find_objects(&[
        Attribute::Class(ObjectClass::PRIVATE_KEY),
        Attribute::Id(want_id.to_vec()),
    ])?;
    handles
        .into_iter()
        .next()
        .ok_or(CieError::PrivateKeyNotFound)
}
