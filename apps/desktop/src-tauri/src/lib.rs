mod error;
mod media;

use error::AppError;
use media::analysis::{analyse_audio, AudioMeasurements};
use media::ffprobe::{inspect_media, MediaSource};

#[tauri::command]
async fn inspect_media_cmd(path: String) -> Result<MediaSource, AppError> {
    inspect_media(&path)
}

#[tauri::command]
async fn analyse_audio_cmd(path: String, duration_seconds: f64) -> Result<AudioMeasurements, AppError> {
    tauri::async_runtime::spawn_blocking(move || analyse_audio(&path, duration_seconds))
        .await
        .map_err(|e| AppError::SystemError(format!("Task spawn error: {}", e)))?
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
    .invoke_handler(tauri::generate_handler![inspect_media_cmd, analyse_audio_cmd])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
