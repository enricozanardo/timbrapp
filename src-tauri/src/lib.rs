pub mod cie;

use base64::Engine;

/// Write raw bytes (base64-encoded over IPC) to an absolute filesystem path
/// chosen by the user through the native save dialog.
#[tauri::command]
fn write_file_bytes(path: String, data_base64: String) -> Result<(), String> {
  let bytes = base64::engine::general_purpose::STANDARD
    .decode(data_base64.as_bytes())
    .map_err(|e| format!("invalid base64: {e}"))?;
  std::fs::write(&path, bytes).map_err(|e| format!("failed to write '{path}': {e}"))
}

/// Read a file from an absolute path (chosen via the native open dialog) and
/// return its bytes base64-encoded for the IPC boundary.
#[tauri::command]
fn read_file_bytes(path: String) -> Result<String, String> {
  let bytes = std::fs::read(&path).map_err(|e| format!("failed to read '{path}': {e}"))?;
  Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    // Updater + process: lets the app check GitHub Releases for newer
    // versions, download and verify the signed update, then relaunch.
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_process::init())
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![
      cie::cie_status,
      cie::cie_list_readers,
      cie::cie_list_certificates,
      cie::cie_is_enrolled,
      cie::cie_enroll,
      cie::cie_sign_raw,
      cie::cie_sign_pdf,
      cie::cie_sign_bytes,
      write_file_bytes,
      read_file_bytes,
    ])
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
