mod error;
mod media;

use error::AppError;
use media::ffprobe::{inspect_media, MediaSource};

#[tauri::command]
async fn inspect_media_cmd(path: String) -> Result<MediaSource, AppError> {
    inspect_media(&path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
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
    .invoke_handler(tauri::generate_handler![inspect_media_cmd])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
