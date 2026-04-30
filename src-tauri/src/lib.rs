#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // v0.1.6: replaced broken <input type="file"> pickers with tauri-plugin-dialog
  // so the native OS file chooser is used directly on all platforms.
  tauri::Builder::default()
    // Updater + process: lets the app check GitHub Releases for newer
    // versions, download and verify the signed update, then relaunch.
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_process::init())
    // Native file dialogs — replaces the broken WebView-based <input type="file">
    // picker that shows a white window on Linux due to a WRY/WebKit2GTK bug where
    // child WebViews don't inherit custom URI scheme handlers.
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_fs::init())
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
