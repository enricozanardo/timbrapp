use serde::Serialize;

/// File data returned to the JS layer after the user picks or drops a file.
#[derive(Serialize)]
struct PickedFile {
  name: String,
  data: Vec<u8>,
}

/// Cross-platform file picker.
///
/// **Linux** — GTK3 file-chooser dialogs conflict with WebKitGTK's own GTK
/// context and render as a blank white window on many systems (confirmed on
/// Gentoo + Intel Arc / Xe GPU).  Instead we spawn Thunar detached and return
/// `None` immediately; the frontend shows a "drag here" hint and the file
/// arrives via Tauri's `onDragDropEvent`.
///
/// **macOS / Windows** — `rfd` opens the native OS dialog (NSOpenPanel /
/// IFileOpenDialog) which has no GTK dependency and works on every system.
/// The picked file's bytes are returned directly so no drag-and-drop step is
/// needed.
#[tauri::command]
async fn pick_file(filter_type: String) -> Result<Option<PickedFile>, String> {
  // ---- Linux: open Thunar, signal the frontend to show the drag hint ----
  #[cfg(target_os = "linux")]
  {
    let _ = &filter_type; // filter applied by the file manager itself
    std::process::Command::new("thunar")
      .spawn()
      .map_err(|e| format!("Failed to open Thunar: {e}"))?;
    return Ok(None);
  }

  // ---- macOS / Windows: native dialog via rfd --------------------------
  #[cfg(not(target_os = "linux"))]
  {
    use rfd::AsyncFileDialog;
    let mut dialog = AsyncFileDialog::new();
    match filter_type.as_str() {
      "pdf" => { dialog = dialog.add_filter("PDF", &["pdf"]); }
      "png" => { dialog = dialog.add_filter("PNG Image", &["png"]); }
      _     => {}
    }
    let handle = dialog.pick_file().await;
    return match handle {
      None    => Ok(None),
      Some(h) => {
        let name = h.file_name();
        let data = h.read().await;
        Ok(Some(PickedFile { name, data }))
      }
    };
  }
}

/// Read a file by absolute path and return its name + raw bytes.
/// Used by `onDragDropEvent` on all platforms to load a dropped file.
#[tauri::command]
fn read_file(path: String) -> Result<PickedFile, String> {
  let data = std::fs::read(&path)
    .map_err(|e| format!("Cannot read '{path}': {e}"))?;
  let name = std::path::Path::new(&path)
    .file_name()
    .and_then(|n| n.to_str())
    .unwrap_or("file")
    .to_string();
  Ok(PickedFile { name, data })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![pick_file, read_file])
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
