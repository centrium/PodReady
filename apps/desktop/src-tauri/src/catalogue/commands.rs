use tauri::State;
use crate::batch::BatchManager;
use crate::catalogue::baseline::ShowBaseline;
use crate::catalogue::models::{
    AddBatchEpisodesResult, AddEpisodeOutcome, CatalogueEpisode, CreateShowInput, MoveEpisodesResult,
    Show, ShowSummary, ShowWithEpisodes, UpdateShowInput,
};
use crate::catalogue::service::CatalogueService;
use crate::catalogue::show_check::ShowCheck;
use crate::error::AppError;
use crate::media::ffprobe::MediaSource;

#[tauri::command]
pub async fn get_shows_cmd(
    catalogue: State<'_, CatalogueService>,
) -> Result<Vec<ShowSummary>, AppError> {
    catalogue.get_shows()
}

#[tauri::command]
pub async fn get_show_cmd(
    id: String,
    catalogue: State<'_, CatalogueService>,
) -> Result<ShowWithEpisodes, AppError> {
    catalogue.get_show(&id)
}

#[tauri::command]
pub async fn get_show_baseline_cmd(
    id: String,
    catalogue: State<'_, CatalogueService>,
) -> Result<ShowBaseline, AppError> {
    catalogue.get_show_baseline(&id)
}

#[tauri::command]
pub async fn get_show_check_for_episode_cmd(
    id: String,
    catalogue: State<'_, CatalogueService>,
) -> Result<ShowCheck, AppError> {
    catalogue.get_show_check_for_episode(&id)
}

#[tauri::command]
pub async fn run_show_check_for_media_cmd(
    show_id: String,
    media: MediaSource,
    catalogue: State<'_, CatalogueService>,
) -> Result<ShowCheck, AppError> {
    catalogue.run_show_check_for_media(&show_id, &media)
}

#[tauri::command]
pub async fn create_show_cmd(
    input: CreateShowInput,
    catalogue: State<'_, CatalogueService>,
) -> Result<Show, AppError> {
    catalogue.create_show(&input.name, input.description.as_deref())
}

#[tauri::command]
pub async fn update_show_cmd(
    input: UpdateShowInput,
    catalogue: State<'_, CatalogueService>,
) -> Result<Show, AppError> {
    catalogue.update_show(&input.id, &input.name, input.description.as_deref())
}

#[tauri::command]
pub async fn delete_show_cmd(
    id: String,
    catalogue: State<'_, CatalogueService>,
) -> Result<(), AppError> {
    catalogue.delete_show(&id)
}

#[tauri::command]
pub async fn add_episode_to_show_cmd(
    show_id: String,
    media: MediaSource,
    catalogue: State<'_, CatalogueService>,
) -> Result<AddEpisodeOutcome, AppError> {
    catalogue.add_media_source_to_show(&show_id, &media)
}

#[tauri::command]
pub async fn add_batch_episodes_to_show_cmd(
    show_id: String,
    job_id: String,
    batch_manager: State<'_, BatchManager>,
    catalogue: State<'_, CatalogueService>,
) -> Result<AddBatchEpisodesResult, AppError> {
    let job = batch_manager.get_job(&job_id)?;

    catalogue.add_batch_episodes_to_show(&show_id, &job.episodes)
}

#[tauri::command]
pub async fn get_catalogue_episode_cmd(
    id: String,
    catalogue: State<'_, CatalogueService>,
) -> Result<CatalogueEpisode, AppError> {
    catalogue.get_episode(&id)
}

#[tauri::command]
pub async fn delete_catalogue_episode_cmd(
    id: String,
    catalogue: State<'_, CatalogueService>,
) -> Result<(), AppError> {
    catalogue.delete_episode(&id)
}

#[tauri::command]
pub async fn delete_catalogue_episodes_cmd(
    episode_ids: Vec<String>,
    catalogue: State<'_, CatalogueService>,
) -> Result<usize, AppError> {
    catalogue.delete_episodes(&episode_ids)
}

#[tauri::command]
pub async fn move_catalogue_episodes_cmd(
    episode_ids: Vec<String>,
    target_show_id: String,
    catalogue: State<'_, CatalogueService>,
) -> Result<MoveEpisodesResult, AppError> {
    catalogue.move_episodes(&episode_ids, &target_show_id)
}

#[tauri::command]
pub async fn reanalyse_catalogue_episode_cmd(
    id: String,
    catalogue: State<'_, CatalogueService>,
) -> Result<CatalogueEpisode, AppError> {
    let catalogue = catalogue.inner().clone();
    tauri::async_runtime::spawn_blocking(move || catalogue.reanalyse_catalogue_episode(&id))
        .await
        .map_err(|e| AppError::SystemError(format!("Task spawn error: {}", e)))?
}

#[tauri::command]
pub async fn relink_catalogue_episode_cmd(
    episode_id: String,
    new_source_path: String,
    catalogue: State<'_, CatalogueService>,
) -> Result<CatalogueEpisode, AppError> {
    let catalogue = catalogue.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        catalogue.relink_and_reanalyse_episode(&episode_id, &new_source_path)
    })
    .await
    .map_err(|e| AppError::SystemError(format!("Task spawn error: {}", e)))?
}

