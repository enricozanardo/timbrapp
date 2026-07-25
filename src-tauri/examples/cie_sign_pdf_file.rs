//! Sign a real PDF file end-to-end (PAdES-B-B) for verification with pdfsig.
//!
//! Usage:
//!   cargo run --example cie_sign_pdf_file -- <module.so> <in.pdf> <out.pdf>
//!
//! Auto-selects the first reader + signing certificate, prompts for the last 4
//! PIN digits (echo off), and writes the signed PDF. WARNING: a wrong PIN
//! consumes one of the card's 3 attempts.

use std::io::{self, Write};

use timbrapp_lib::cie::{pades, pkcs11};

fn main() {
    let mut args = std::env::args().skip(1);
    let module_path = args.next().unwrap_or_else(default_module_path);
    let in_pdf = args.next().expect("usage: <module.so> <in.pdf> <out.pdf>");
    let out_pdf = args.next().expect("usage: <module.so> <in.pdf> <out.pdf>");

    let readers = pkcs11::list_readers(&module_path).expect("list_readers");
    let slot_id = readers.first().expect("no CIE on reader").slot_id;
    let certs = pkcs11::list_certificates(&module_path, slot_id).expect("list_certificates");
    let cert = certs
        .iter()
        .find(|c| c.key_usage_sign)
        .or_else(|| certs.first())
        .expect("no certificate");
    println!("Slot {slot_id}, cert '{}' ({})", cert.label, cert.subject);

    let pin = read_pin("Last 4 digits of PIN: ");
    assert!(!pin.is_empty(), "empty PIN");

    let pdf = std::fs::read(&in_pdf).expect("read input pdf");
    // Draw a visible box near the bottom-left of the first page (PDF points).
    let appearance = pades::SignatureAppearance {
        page_index: 0,
        rect: [40.0, 40.0, 300.0, 110.0],
        lines: vec![
            "Firmato digitalmente da".to_string(),
            cert.subject.clone(),
            "Firma Elettronica Avanzata (CIE)".to_string(),
        ],
    };
    let signed = pades::sign_pdf(
        &pdf,
        &module_path,
        slot_id,
        &cert.id_hex,
        &pin,
        Some("Test signature"),
        None,
        Some(appearance),
    )
    .expect("sign_pdf");
    std::fs::write(&out_pdf, &signed).expect("write output");
    println!("Wrote {} ({} bytes)", out_pdf, signed.len());
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
    "/usr/local/lib/libcie-pkcs11.so".to_string()
}
