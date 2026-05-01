use serde::Serialize;

/// Read a file by absolute path and return its name + raw bytes.
///
/// Used by the drag-and-drop handler: WebKitGTK on Linux passes file drops as
/// `text/uri-list` (e.g. `file:///home/user/doc.pdf`) rather than populating
/// `DataTransfer.files`.  The frontend extracts the path from the URI and
/// calls this command to get the file bytes without any file-chooser dialog.
#[derive(Serialize)]
struct FileData {
  name: String,
  data: Vec<u8>,
}

#[tauri::command]
fn read_file(path: String) -> Result<FileData, String> {
  let data = std::fs::read(&path)
    .map_err(|e| format!("Cannot read '{path}': {e}"))?;
  let name = std::path::Path::new(&path)
    .file_name()
    .and_then(|n| n.to_str())
    .unwrap_or("file")
    .to_string();
  Ok(FileData { name, data })
}

/// Open Thunar (the file manager) so the user can navigate to a file and
/// drag it into the app's drop zone.  This is the Linux workaround for the
/// WRY/WebKitGTK bug where any file-chooser dialog (WebView child window or
/// in-process GTK dialog) renders as a blank white window on systems with
/// Intel Arc / Xe GPUs.
///
/// Thunar is spawned detached — we return immediately without waiting for it
/// to exit so the Tauri window stays fully responsive.
#[tauri::command]
fn open_file_manager() -> Result<(), String> {
  std::process::Command::new("thunar")
    .spawn()
    .map_err(|e| format!("Failed to open Thunar: {e}"))?;
  Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    // Updater + process: lets the app check GitHub Releases for newer
    // versions, download and verify the signed update, then relaunch.
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_process::init())
    .invoke_handler(tauri::generate_handler![open_file_manager, read_file])
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
