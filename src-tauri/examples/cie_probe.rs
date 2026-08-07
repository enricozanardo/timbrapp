//! Standalone probe to validate a CIE PKCS#11 module loads and initializes,
//! independent of the Tauri app / reader. Prints library info and slot counts.
//!
//! Usage: cargo run --example cie_probe -- /path/to/libcie-pkcs11.so

use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: cie_probe <path-to-pkcs11-module>");

    println!("Loading module: {path}");
    let pkcs11 = match Pkcs11::new(&path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("FAILED to load module: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = pkcs11.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK)) {
        eprintln!("FAILED C_Initialize: {e}");
        std::process::exit(2);
    }
    println!("C_Initialize: OK (module loaded and initialized)");

    match pkcs11.get_library_info() {
        Ok(info) => println!(
            "Library: manufacturer='{}' description='{}' version={}.{}",
            info.manufacturer_id(),
            info.library_description(),
            info.library_version().major(),
            info.library_version().minor()
        ),
        Err(e) => println!("get_library_info error: {e}"),
    }

    let slots = match pkcs11.get_slots_with_token() {
        Ok(slots) => slots,
        Err(e) => {
            println!(
                "get_slots_with_token error: {e}\n(this usually means pcscd is not running or no reader is attached)"
            );
            return;
        }
    };
    println!("Readers with a card present: {}", slots.len());

    for slot in slots {
        if let Ok(si) = pkcs11.get_slot_info(slot) {
            println!("  - slot {}: {}", slot.id(), si.slot_description().trim());
        }
        list_certs(&pkcs11, slot);
    }
}

fn list_certs(pkcs11: &Pkcs11, slot: cryptoki::slot::Slot) {
    use cryptoki::object::{Attribute, AttributeType, ObjectClass};
    use der::Decode;

    let session = match pkcs11.open_ro_session(slot) {
        Ok(s) => s,
        Err(e) => {
            println!("    open_ro_session error: {e}");
            return;
        }
    };

    let handles = match session.find_objects(&[Attribute::Class(ObjectClass::CERTIFICATE)]) {
        Ok(h) => h,
        Err(e) => {
            println!("    find certificates error: {e}");
            return;
        }
    };
    println!("    certificates: {}", handles.len());

    for h in handles {
        let attrs = session
            .get_attributes(h, &[AttributeType::Value, AttributeType::Label, AttributeType::Id])
            .unwrap_or_default();
        let mut label = String::new();
        let mut id = Vec::new();
        let mut value = Vec::new();
        for a in attrs {
            match a {
                Attribute::Label(v) => label = String::from_utf8_lossy(&v).into_owned(),
                Attribute::Id(v) => id = v,
                Attribute::Value(v) => value = v,
                _ => {}
            }
        }
        let subject = x509_cert::Certificate::from_der(&value)
            .map(|c| c.tbs_certificate.subject.to_string())
            .unwrap_or_else(|_| "<unparseable>".into());
        println!(
            "      - id={} label='{}' subject={}",
            hex::encode(&id),
            label,
            subject
        );
    }
}
