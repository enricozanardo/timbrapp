//! Standalone, safe validation of CIE enrollment ("Abilita CIE").
//!
//! Usage:
//!   cargo run --example cie_enroll -- /path/to/libcie-pkcs11.so
//!
//! It will:
//!   1. list readers and pick the first slot with a CIE present;
//!   2. do a PIN-free "already enrolled?" check and STOP if already enrolled;
//!   3. otherwise prompt for the full 8-digit PIN (echo disabled) and call
//!      `AbilitaCIE` exactly ONCE (no retries).
//!
//! WARNING: a wrong PIN consumes one of the card's 3 attempts (then PUK needed).

use std::io::{self, Write};

use timbrapp_lib::cie::{enroll, pkcs11};

fn main() {
    let module_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| default_module_path());

    println!("Module: {module_path}\n");

    let readers = match pkcs11::list_readers(&module_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to list readers: {e}");
            std::process::exit(1);
        }
    };
    if readers.is_empty() {
        eprintln!("No reader has a CIE inserted. Place the card on the NFC reader and retry.");
        std::process::exit(1);
    }
    let slot_id = readers[0].slot_id;
    println!(
        "Using slot {} - {}",
        slot_id,
        readers[0].slot_description
    );

    match pkcs11::is_enrolled(&module_path, slot_id) {
        Ok(true) => {
            println!("\nThis CIE is ALREADY enrolled. Nothing to do - you can sign directly.");
            return;
        }
        Ok(false) => println!("\nThis CIE is NOT enrolled yet."),
        Err(e) => {
            eprintln!("Enrollment pre-check failed: {e}");
            std::process::exit(1);
        }
    }

    println!(
        "\n*** WARNING ***\nA WRONG PIN consumes one of the card's 3 attempts (then the PUK is\n\
         required). Enter your FULL 8-digit CIE PIN carefully. This runs ONCE."
    );
    print!("\nType 'yes' to proceed: ");
    io::stdout().flush().ok();
    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm).ok();
    if confirm.trim() != "yes" {
        println!("Aborted - no PIN sent to the card.");
        return;
    }

    let pin = read_pin("Full 8-digit CIE PIN: ");
    if pin.len() != 8 || !pin.bytes().all(|b| b.is_ascii_digit()) {
        eprintln!("That is not 8 digits. Aborting WITHOUT contacting the card.");
        std::process::exit(1);
    }

    println!("\nEnrolling (keep the card on the reader)...");
    match enroll::abilita_cie(&module_path, &pin) {
        Ok(outcome) => {
            println!("\n--- result ---");
            println!("code:            {}", outcome.code);
            println!("ok:              {}", outcome.ok);
            println!("consumed attempt:{}", outcome.consumed_attempt);
            println!("message:         {}", outcome.message);
            if let Some(p) = &outcome.pan {
                println!("pan:             {p}");
            }
            if let Some(c) = &outcome.cardholder {
                println!("cardholder:      {c}");
            }
            if let Some(s) = &outcome.serial {
                println!("serial:          {s}");
            }
            if let Some(a) = outcome.attempts_left {
                println!("attempts left:   {a}");
            }
            if outcome.ok {
                println!("\nVerifying enrollment via a fresh session...");
                match pkcs11::is_enrolled(&module_path, slot_id) {
                    Ok(true) => println!("Confirmed: the card is now enrolled and readable."),
                    Ok(false) => println!("Hmm - still not readable. Try re-running the probe."),
                    Err(e) => println!("Verify failed: {e}"),
                }
            }
        }
        Err(e) => eprintln!("Enrollment call failed: {e}"),
    }
}

fn read_pin(prompt: &str) -> String {
    print!("{prompt}");
    io::stdout().flush().ok();
    // Best-effort: disable terminal echo while typing the PIN.
    let echo_off = std::process::Command::new("stty").arg("-echo").status().is_ok();
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
