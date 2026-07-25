//! One-time CIE enrollment ("Abilita CIE" / pairing).
//!
//! Over the contactless interface the CIE middleware must pair with a specific
//! card before any PKCS#11 session can read or sign - until then every read
//! fails with a "BER decode error". The official CIEID app performs this via the
//! vendor-exported `AbilitaCIE` function; we call the exact same entry point so
//! users never need to install the separate Java "Software CIE" GUI.
//!
//! Reverse-engineered contract (from the official `cieid.jar` JNA bindings):
//!
//! ```text
//! int AbilitaCIE(const char* unused /*null*/,
//!                const char* pin     /*full 8-digit CIE PIN*/,
//!                int*        attempts /*out, best-effort*/,
//!                void (*progress)(int percent, const char* message),
//!                void (*completed)(const char* pan,
//!                                  const char* cardholder,
//!                                  const char* ef_seriale)) -> int
//! ```
//!
//! Return codes: 0 = OK, 240 = already enrolled (safe no-op), 160 = WRONG PIN
//! (consumes one of the 3 attempts!), 164 = PIN blocked, 224/225 = no card,
//! 5 = smart-card communication error.
//!
//! SAFETY: a wrong PIN permanently consumes an attempt, so this is called
//! exactly once per explicit user action, never retried, and only after a
//! PIN-free "already enrolled?" pre-check ([`super::pkcs11::is_enrolled`]).

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CStr, CString};

use libloading::{Library, Symbol};
use serde::Serialize;

use crate::cie::error::{CieError, CieResult};

/// Outcome of an [`abilita_cie`] call, serialized to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrollOutcome {
    /// Raw vendor return code (0 = OK, 240 = already enrolled, ...).
    pub code: i32,
    /// True for success (0) or already-enrolled (240).
    pub ok: bool,
    /// True specifically when a wrong PIN consumed an attempt (code 160).
    pub consumed_attempt: bool,
    pub message: String,
    pub pan: Option<String>,
    pub cardholder: Option<String>,
    pub serial: Option<String>,
    /// Remaining PIN attempts as reported by the module, when available.
    pub attempts_left: Option<i32>,
}

thread_local! {
    static COMPLETED: RefCell<Option<(String, String, String)>> = const { RefCell::new(None) };
    static LAST_PROGRESS: RefCell<Option<(i32, String)>> = const { RefCell::new(None) };
}

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

extern "C" fn progress_cb(progress: c_int, message: *const c_char) {
    let msg = unsafe { cstr(message) };
    LAST_PROGRESS.with(|p| *p.borrow_mut() = Some((progress as i32, msg)));
}

extern "C" fn completed_cb(
    pan: *const c_char,
    cardholder: *const c_char,
    serial: *const c_char,
) {
    let v = unsafe { (cstr(pan), cstr(cardholder), cstr(serial)) };
    COMPLETED.with(|c| *c.borrow_mut() = Some(v));
}

type ProgressCb = extern "C" fn(c_int, *const c_char);
type CompletedCb = extern "C" fn(*const c_char, *const c_char, *const c_char);
type AbilitaCieFn =
    unsafe extern "C" fn(*const c_char, *const c_char, *mut c_int, ProgressCb, CompletedCb) -> c_int;

fn code_message(code: i32) -> (bool, bool, String) {
    // (ok, consumed_attempt, message)
    match code {
        0 => (true, false, "CIE enrolled successfully.".into()),
        240 => (true, false, "Card was already enrolled.".into()),
        160 => (
            false,
            true,
            "Wrong PIN. WARNING: this used one of the 3 attempts before the card locks with the PUK."
                .into(),
        ),
        164 => (
            false,
            false,
            "The CIE PIN is blocked. Unblock it with the PUK before enrolling.".into(),
        ),
        224 | 225 => (
            false,
            false,
            "No CIE detected on the reader. Place the card on the NFC reader and retry.".into(),
        ),
        5 => (
            false,
            false,
            "Unexpected error communicating with the smart card. Reposition the card and retry."
                .into(),
        ),
        other => (
            false,
            false,
            format!("Enrollment failed (vendor error {other})."),
        ),
    }
}

/// Enroll (pair) the CIE using the full 8-digit PIN. Calls the vendor
/// `AbilitaCIE` exactly once - never retries.
///
/// Callers MUST gate this behind explicit user intent and a prior
/// [`super::pkcs11::is_enrolled`] check, because a wrong PIN (code 160)
/// consumes one of the card's 3 attempts.
pub fn abilita_cie(module_path: &str, pin: &str) -> CieResult<EnrollOutcome> {
    if pin.len() != 8 || !pin.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CieError::Other(
            "Enrollment requires the full 8-digit CIE PIN.".into(),
        ));
    }

    COMPLETED.with(|c| *c.borrow_mut() = None);
    LAST_PROGRESS.with(|p| *p.borrow_mut() = None);

    let pin_c =
        CString::new(pin).map_err(|_| CieError::Other("PIN contains a NUL byte".into()))?;
    let mut attempts: c_int = -1;

    let code: i32 = unsafe {
        let lib = Library::new(module_path)
            .map_err(|e| CieError::Other(format!("failed to load module '{module_path}': {e}")))?;
        let func: Symbol<AbilitaCieFn> = lib
            .get(b"AbilitaCIE\0")
            .map_err(|e| CieError::Other(format!("module has no AbilitaCIE symbol: {e}")))?;
        func(
            std::ptr::null(),
            pin_c.as_ptr(),
            &mut attempts as *mut c_int,
            progress_cb,
            completed_cb,
        ) as i32
    };

    let (ok, consumed_attempt, message) = code_message(code);
    let (pan, cardholder, serial) = match COMPLETED.with(|c| c.borrow_mut().take()) {
        Some((p, c, s)) => (Some(p), Some(c), Some(s)),
        None => (None, None, None),
    };
    let attempts_left = if attempts >= 0 {
        Some(attempts as i32)
    } else {
        None
    };

    Ok(EnrollOutcome {
        code,
        ok,
        consumed_attempt,
        message,
        pan,
        cardholder,
        serial,
        attempts_left,
    })
}
