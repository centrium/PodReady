mod assessment;
mod batch;
mod catalogue;
mod error;
mod export;
mod fixplan;
mod media;
mod transcription;

#[cfg(test)]
pub(crate) static TEST_GLOBAL_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

use tauri::Manager;
use assessment::{assess_media, Assessment};
use batch::{
    cancel_batch_analysis_cmd, get_batch_job_cmd, select_files_cmd, start_batch_analysis_cmd,
    BatchManager,
};
use catalogue::{
    add_batch_episodes_to_show_cmd, add_episode_to_show_cmd, create_show_cmd,
    delete_catalogue_episode_cmd, delete_show_cmd, get_catalogue_episode_cmd, get_show_cmd,
    get_shows_cmd, update_show_cmd, CatalogueRepository, CatalogueService,
};
use error::AppError;
use export::{create_publishing_package, ExportOptions, PodReadyPackage, ReportActionItem};
use fixplan::{generate_fix_plan, FixPlan};
use media::analysis::{analyse_audio, AudioMeasurements};
use media::ffprobe::{inspect_media, MediaFormat, MediaInspection, MediaSource};
use media::processing::{execute_fix_plan, ProcessAudioResponse};
use transcription::{transcribe_audio, TranscriptResult};

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

#[tauri::command]
fn assess_media_cmd(
    inspection: MediaInspection,
    measurements: Option<AudioMeasurements>,
    format: MediaFormat,
    codec: String,
) -> Result<Assessment, AppError> {
    Ok(assess_media(
        &inspection,
        measurements.as_ref(),
        &format,
        &codec,
    ))
}

#[tauri::command]
fn generate_fix_plan_cmd(assessment: Assessment) -> Result<FixPlan, AppError> {
    Ok(generate_fix_plan(&assessment))
}

#[tauri::command]
async fn process_audio_cmd(
    source_path: String,
    plan: FixPlan,
    before_measurements: Option<AudioMeasurements>,
    before_assessment: Option<Assessment>,
) -> Result<ProcessAudioResponse, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        execute_fix_plan(&source_path, &plan, before_measurements, before_assessment)
    })
    .await
    .map_err(|e| AppError::SystemError(format!("Task spawn error: {}", e)))?
}

#[tauri::command]
async fn transcribe_audio_cmd(audio_path: String) -> Result<TranscriptResult, AppError> {
    tauri::async_runtime::spawn_blocking(move || transcribe_audio(&audio_path, None))
        .await
        .map_err(|e| AppError::SystemError(format!("Task spawn error: {}", e)))?
}

#[tauri::command]
async fn export_package_cmd(
    input_audio_path: String,
    source_original_path: String,
    options: ExportOptions,
    before_measurements: Option<AudioMeasurements>,
    before_assessment: Option<Assessment>,
    applied_actions: Vec<ReportActionItem>,
) -> Result<PodReadyPackage, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        create_publishing_package(
            &input_audio_path,
            &source_original_path,
            &options,
            before_measurements,
            before_assessment,
            applied_actions,
        )
    })
    .await
    .map_err(|e| AppError::SystemError(format!("Task spawn error: {}", e)))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .manage(BatchManager::new())
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      let app_data_dir = app.path().app_data_dir().map_err(|e| {
        Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get app data dir: {}", e)))
      })?;
      let db_path = app_data_dir.join("podready_catalogue.db");
      let repo = CatalogueRepository::open_file(&db_path).map_err(|e| {
        Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to open catalogue database at {:?}: {}", db_path, e)))
      })?;
      let catalogue_service = CatalogueService::new(repo);
      app.manage(catalogue_service);

      media::binaries::start_background_model_verification();
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
        inspect_media_cmd,
        analyse_audio_cmd,
        assess_media_cmd,
        generate_fix_plan_cmd,
        process_audio_cmd,
        transcribe_audio_cmd,
        export_package_cmd,
        start_batch_analysis_cmd,
        cancel_batch_analysis_cmd,
        get_batch_job_cmd,
        select_files_cmd,
        get_shows_cmd,
        get_show_cmd,
        create_show_cmd,
        update_show_cmd,
        delete_show_cmd,
        add_episode_to_show_cmd,
        add_batch_episodes_to_show_cmd,
        get_catalogue_episode_cmd,
        delete_catalogue_episode_cmd
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}


