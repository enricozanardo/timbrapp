//! Controlled validation of the on-card signing path (post-enrollment).
//!
//! Usage:
//!   cargo run --example cie_sign_test -- /path/to/libcie-pkcs11.so
//!
//! It lists the signing certificate, then C_Login's with the PIN you type and
//! signs a fixed test message. For an already-paired CIE the module expects only
//! the LAST 4 digits of the PIN (it stores the first 4 during enrollment).
//!
//! WARNING: a wrong PIN consumes one of the card's 3 attempts.

use std::io::{self, Write};

use timbrapp_lib::cie::pkcs11::{self, CardSigner};

fn main() {
    let module_path = std::env::args()
        .nth(1)
        .unwrap_or_else(default_module_path);
    println!("Module: {module_path}\n");

    let readers = match pkcs11::list_readers(&module_path) {
        Ok(r) if !r.is_empty() => r,
        Ok(_) => {
            eprintln!("No CIE on the reader.");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("list_readers failed: {e}");
            std::process::exit(1);
        }
    };
    let slot_id = readers[0].slot_id;
    println!("Slot {slot_id}: {}", readers[0].slot_description);

    let certs = match pkcs11::list_certificates(&module_path, slot_id) {
        Ok(c) if !c.is_empty() => c,
        Ok(_) => {
            eprintln!("No certificate found - is the card enrolled?");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("list_certificates failed: {e}");
            std::process::exit(1);
        }
    };
    let cert = &certs[0];
    println!("Cert: label='{}' id={}", cert.label, cert.id_hex);
    println!("      subject={}", cert.subject);
    println!(
        "      keyUsage(sign)={} notAfter={}",
        cert.key_usage_sign, cert.not_after
    );

    println!(
        "\nFor a paired CIE, enter the LAST 4 digits of your PIN.\n\
         (A wrong PIN consumes one of the 3 attempts.)"
    );
    let pin = read_pin("Last 4 digits of PIN: ");
    if pin.is_empty() {
        eprintln!("Empty PIN, aborting without contacting the card.");
        std::process::exit(1);
    }

    println!("\nLogging in and signing a test message...");
    match CardSigner::open(&module_path, slot_id, &cert.id_hex, &pin) {
        Ok(signer) => {
            let msg = b"TimbrApp CIE sign self-test";
            match signer.sign(msg) {
                Ok(sig) => {
                    println!("\nSUCCESS: on-card signature produced ({} bytes).", sig.len());
                    let head: Vec<String> =
                        sig.iter().take(16).map(|b| format!("{b:02X}")).collect();
                    println!("sig[0..16]: {}", head.join(" "));
                    println!("cert DER: {} bytes", signer.certificate_der().len());
                    println!("\n=> PIN format 'last 4 digits' CONFIRMED. Signing works.");
                }
                Err(e) => eprintln!("C_Sign failed: {e}"),
            }
        }
        Err(e) => eprintln!("Login/open failed: {e}"),
    }
}

fn read_pin(prompt: &str) -> String {
    print!("{prompt}");
    io::stdout().flush().ok();
    let echo_off = std::process::Command::new("stty")
        .arg("-echo")
        .status()
        .is_ok();
    let mut pin = String::new();
    io::stdin().read_line(&mut pin).ok();
    if echo_off {
        let _ = std::process::Command::new("stty").arg("echo").status();
        println!();
    }
    pin.trim().to_string()
}

fn default_module_path() -> String {
    if let Ok(home) = std::env::var("HOME") {
        format!("{home}/.local/lib/cie/libcie-pkcs11.so")
    } else {
        "/usr/local/lib/libcie-pkcs11.so".to_string()
    }
}
