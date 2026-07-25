# CIE local signing (PAdES / CAdES)

TimbrApp can sign documents locally with the Italian **Carta d'Identita
Elettronica (CIE)** using the official CieSign "Desktop" model: a
contactless/NFC reader talks to the CIE chip through the official CIE PKCS#11
middleware, and TimbrApp assembles a **PAdES** (PDF) or **CAdES** (any file)
signature around the on-card signature.

This is a **Firma Elettronica Avanzata (FEA)** under eIDAS art. 26 - no QTSP and
no cloud. It is legally valid for the generality of documents (CAD art. 20
co. 1-bis), **except** the acts listed in art. 1350 c.c. (e.g. real-estate
transfers), which require a qualified signature (FEQ) from a QTSP.

## Hardware

- The CIE 3.0 chip is **contactless only** (ISO 14443 A/B, 13.56 MHz). A contact
  chip reader cannot read it.
- Use a **USB contactless / NFC PC-SC reader**. Recommended: **Bit4id miniLector
  AIR** (purpose-built for the CIE). Avoid the ACR122U (end-of-life, unreliable
  with the CIE: no extended-APDU / time-extension support) and any contact-only
  reader (e.g. Bit4id miniLector EVO).
- Integration is reader-agnostic: it goes through the CIE PKCS#11 module, so any
  PC-SC contactless reader recognized by the OS will work.

## Software prerequisites (on the machine that runs TimbrApp)

1. Install the official **"Software CIE"** (Ministero dell'Interno / IPZS) from
   <https://www.cartaidentita.interno.gov.it>. It installs the CIE drivers and a
   PKCS#11 module.
2. Ensure the OS smart-card service is running:
   - Windows: the **Smart Card** service (`SCardSvr`).
   - Linux: `pcscd`.
3. Plug in the contactless reader and place the CIE on it. Confirm the card is
   detected (e.g. with the Software CIE tool) before using TimbrApp.
4. Point TimbrApp at the PKCS#11 module if it is not auto-detected (see below).

### PKCS#11 module resolution order

The backend resolves the module path in this order:

1. An **explicit path** typed into the sign dialog / diagnostics panel.
2. A module **bundled inside TimbrApp** under `resources/pkcs11/` (see below).
3. A **system-installed** module (official "Software CIE" default locations).

## Enrollment (one-time "Abilita CIE" pairing)

The CIE contactless interface must be **paired** with the middleware once per
card, per machine/user profile, before it can be read or used to sign. Until a
card is enrolled, opening a PKCS#11 session fails (`BER decode error` /
`token not recognized`) because the secure channel can't be established.

TimbrApp performs this **in-app**, so users never need the official Java GUI
(`cieid.jar`). The backend calls the middleware's own headless `AbilitaCIE`
entry point directly:

- **`cie_is_enrolled`** - PIN-free check (does a session open + read a cert?).
  The sign dialog runs this automatically after selecting a reader.
- **`cie_enroll`** - one-time pairing. Requires the **full 8-digit PIN**
  (first 4 from issuance + last 4 from the letter). Calls `AbilitaCIE` **once,
  no retries**. Return codes: `0` = OK, `240` = already enrolled (safe no-op),
  `160` = **wrong PIN (consumes one of the 3 attempts!)**, `164` = PIN blocked,
  `224/225` = no card, `5` = comms error.

**PIN model:** enrollment stores the first 4 digits locally (under `~/.CIEPKI`
on Linux), so **signing afterwards only needs the last 4 digits**. The sign
dialog collects 8 digits for enrollment and 4 digits for signing accordingly.

> Note: when a third-party app opens a session on an *un-enrolled* card, the
> middleware may try to auto-launch its Java GUI (`it.ipzs.cieid.MainApplication`).
> If that app isn't installed the spawn fails harmlessly in the background - it
> does not affect TimbrApp's headless enrollment, which never uses the GUI.

You can validate enrollment + signing independently of the app with the bundled
examples (each prompts for the PIN with terminal echo disabled and never
retries):

```sh
cargo run --example cie_enroll     -- /usr/local/lib/libcie-pkcs11.so   # full 8-digit PIN
cargo run --example cie_sign_test  -- /usr/local/lib/libcie-pkcs11.so   # last 4 digits
```

## Shipping TimbrApp without a separate "Software CIE" install

Because the module is loaded dynamically at runtime, you can bundle an
open-source CIE PKCS#11 library inside the app so users install **only
TimbrApp**. The backend auto-discovers it from the app's resource directory.

Steps:

1. Obtain/build the CIE PKCS#11 library for each target you ship, from an
   open-source project such as `italia/cie-middleware` (`cie_sign_sdk`) or
   `M0Rf30/cie-middleware-linux` (opencie). Review its license (EUPL/BSD) and
   ship the required notices.
2. Place the library under `src-tauri/resources/pkcs11/` using the expected
   filename per platform:
   - Windows: `cie-pkcs11.dll` (also tried: `libcie-pkcs11.dll`, `bit4p11.dll`)
   - macOS: `libcie-pkcs11.dylib`
   - Linux: `libcie-pkcs11.so`
   Include any of the module's own runtime dependencies (e.g. OpenSSL) alongside it.
3. Add the folder to `src-tauri/tauri.conf.json` under `bundle.resources`, e.g.:

   ```json
   "bundle": {
     "resources": ["resources/pkcs11/*"]
   }
   ```

4. Rebuild. `cie_status` will now report the bundled module and users won't need
   the official installer.

**Still required regardless of bundling:** the OS PC/SC service (`SCardSvr` on
Windows / `pcscd` on Linux) and the reader's CCID driver (generic, usually
plug-and-play). These are provided by the OS, not by "Software CIE".

If you prefer not to bundle, users can instead install the official "Software
CIE" and TimbrApp will fall back to the system module automatically.

## Using the official CIE Middleware in WSL2 (no "Software CIE" GUI)

The official Linux package `CIE-Middleware-*.amd64.deb` ships the PKCS#11 module
`libcie-pkcs11.so` (manufacturer IPZS). You do **not** need to run its Java GUI
(`cieid.jar`) - TimbrApp loads the `.so` directly. Verified working in WSL2:

```
C_Initialize: OK
Library: manufacturer='IPZS' description='CIE PKCS11' version=1.0
```

Setup:

1. Install the PC/SC runtime the module depends on:
   ```sh
   sudo apt-get install -y pcscd libccid pcsc-tools
   ```
2. Provide the module at a path TimbrApp auto-detects (the deb's default
   location, already in the Linux candidate list):
   ```sh
   dpkg-deb -x CIE-Middleware-*.amd64.deb /tmp/cie
   sudo cp /tmp/cie/usr/local/lib/libcie-pkcs11.so /usr/local/lib/
   ```
   (or install the deb normally with `sudo apt install ./CIE-Middleware-*.deb`).
3. Make sure `pcscd` is running: `sudo pcscd` (or `sudo systemctl enable --now pcscd`).
   If `pcsc_scan` / `C_Initialize` reports `SCardEstablishContext: Access denied`,
   restart it without polkit: `sudo pkill pcscd; sudo pcscd --disable-polkit`.
4. **Attach the USB reader to WSL2**: WSL2 has no direct USB access, so use
   [`usbipd-win`](https://github.com/dorssel/usbipd-win) on the Windows host:
   ```powershell
   usbipd list
   usbipd bind --busid <busid>
   usbipd attach --wsl --busid <busid>
   ```
   Then in WSL confirm the reader appears with `pcsc_scan`.
5. Place the CIE on the reader; `cie_status` will list it. The first time a given
   card is used it must be enrolled once (see
   [Enrollment](#enrollment-one-time-abilita-cie-pairing)); afterwards signing
   works with the last 4 PIN digits.

The module writes its config/log under `~/.CIEPKI/` and `ciepki.ini` next to the
`.so`; make sure those locations are writable by the user running TimbrApp.

You can validate the module independently of the GUI with the bundled probe:

```sh
cargo run --example cie_probe -- /usr/local/lib/libcie-pkcs11.so
```

Note: running the Tauri app itself under WSL2 uses WSLg for the window. Testing
natively on Windows (with the Windows CIE PKCS#11 module) is also fine and avoids
USB passthrough entirely.

## Development note (important)

The smart-card path can only be exercised by a **native build** of TimbrApp
running on the host OS (Windows/macOS/Linux) where the reader and smart-card
service live. It **cannot** be tested from inside **WSL2**, which has no USB /
PC-SC access. Develop/edit in WSL, but run `npm run tauri:build` (or
`npm run tauri:dev` on the native host) to test signing against a real card.

The `cryptoki` crate loads the PKCS#11 module at runtime via `libloading`, so
the project builds fine in WSL even without any smart-card libraries installed.

## How it works

```
Svelte UI  ->  Tauri commands (Rust)  ->  cryptoki (PKCS#11)  ->  CIE chip
                                       ->  lopdf (PAdES placeholder + ByteRange)
                                       ->  RustCrypto cms (SignedData, PAdES-B-B)
```

Commands exposed to the frontend (see `src/lib/cie/api.ts`):

- `cie_status` - locate the PKCS#11 module and list readers with a card.
- `cie_list_readers` - list PC-SC slots that have a card.
- `cie_is_enrolled` - PIN-free check whether the card is already paired.
- `cie_enroll` - one-time enrollment ("Abilita CIE") with the full 8-digit PIN.
- `cie_list_certificates` - list certificates on the card (FEA/nonRepudiation
  first).
- `cie_sign_raw` - on-card `CKM_SHA256_RSA_PKCS` signature of arbitrary bytes
  (a proof-of-concept to validate the key + PIN).
- `cie_sign_pdf` - PAdES-B-B signature of a PDF.
- `cie_sign_bytes` - detached CAdES-B-B (`.p7s`) signature of any file.

## Validating a signed PDF

After signing, verify the output with an external validator:

- the official CIE verification tool, or
- the EU DSS validation demo (<https://ec.europa.eu/digital-building-blocks/DSS/webapp-demo/validation>), or
- `pdfsig signed.pdf` (from poppler-utils).

A valid result reports an AdES/FEA signature and no "document altered" warning.

## Current limitations

- RSA CIE keys only (the CIE uses RSA-2048). EC keys are rejected with a clear
  error.
- PAdES-B-B / CAdES-B-B baseline only. No timestamp / LTV (PAdES-B-LT/LTA) yet.
- Full re-save signature (not an incremental update); the whole output file is
  covered by the ByteRange.
