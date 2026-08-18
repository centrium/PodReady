use rusqlite::{params, Connection, Row};
use crate::assessment::Assessment;
use crate::catalogue::migrations::run_migrations;
use crate::catalogue::models::{
    CatalogueEpisode, MoveEpisodeOutcome, MoveEpisodeOutcomeStatus, MoveEpisodesResult, Show,
    ShowSummary, SourceAvailability,
};
use crate::error::AppError;
use crate::media::ffprobe::MediaFormat;

pub struct CatalogueRepository {
    conn: Connection,
}

impl CatalogueRepository {
    pub fn new(mut conn: Connection) -> Result<Self, AppError> {
        run_migrations(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn open_file(path: &std::path::Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::DatabaseError(format!("Failed to create catalogue directory: {}", e))
            })?;
        }
        let conn = Connection::open(path).map_err(|e| {
            AppError::DatabaseError(format!("Failed to open SQLite database: {}", e))
        })?;
        Self::new(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, AppError> {
        let conn = Connection::open_in_memory().map_err(|e| {
            AppError::DatabaseError(format!("Failed to open in-memory database: {}", e))
        })?;
        Self::new(conn)
    }

    pub fn create_show(
        &mut self,
        id: &str,
        name: &str,
        description: Option<&str>,
        now: &str,
    ) -> Result<Show, AppError> {
        self.conn
            .execute(
                "INSERT INTO shows (id, name, description, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, name, description, now, now],
            )
            .map_err(|e| AppError::DatabaseError(format!("Failed to insert show: {}", e)))?;

        Ok(Show {
            id: id.to_string(),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    pub fn update_show(
        &mut self,
        id: &str,
        name: &str,
        description: Option<&str>,
        now: &str,
    ) -> Result<Show, AppError> {
        let rows_affected = self
            .conn
            .execute(
                "UPDATE shows SET name = ?1, description = ?2, updated_at = ?3 WHERE id = ?4",
                params![name, description, now, id],
            )
            .map_err(|e| AppError::DatabaseError(format!("Failed to update show: {}", e)))?;

        if rows_affected == 0 {
            return Err(AppError::NotFound(format!("Show {} not found", id)));
        }

        self.get_show_by_id(id)?
            .ok_or_else(|| AppError::NotFound(format!("Show {} not found", id)))
    }

    pub fn get_shows(&self) -> Result<Vec<ShowSummary>, AppError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.id, s.name, s.description, s.created_at, s.updated_at,
                        COUNT(e.id) as episode_count,
                        MAX(e.analysed_at) as last_analysed_at
                 FROM shows s
                 LEFT JOIN episodes e ON e.show_id = s.id
                 GROUP BY s.id
                 ORDER BY s.updated_at DESC",
            )
            .map_err(|e| AppError::DatabaseError(format!("Failed to prepare get_shows: {}", e)))?;

        let rows = stmt
            .query_map([], |row| {
                let count: i64 = row.get(5)?;
                Ok(ShowSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    episode_count: count as usize,
                    last_analysed_at: row.get(6)?,
                })
            })
            .map_err(|e| AppError::DatabaseError(format!("Failed to query shows: {}", e)))?;

        let mut shows = Vec::new();
        for r in rows {
            shows.push(r.map_err(|e| AppError::DatabaseError(format!("Row error: {}", e)))?);
        }

        Ok(shows)
    }

    pub fn get_show_by_id(&self, id: &str) -> Result<Option<Show>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, description, created_at, updated_at FROM shows WHERE id = ?1")
            .map_err(|e| AppError::DatabaseError(format!("Failed to prepare get_show_by_id: {}", e)))?;

        let mut rows = stmt
            .query_map(params![id], |row| {
                Ok(Show {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(|e| AppError::DatabaseError(format!("Failed to query show: {}", e)))?;

        if let Some(r) = rows.next() {
            Ok(Some(r.map_err(|e| AppError::DatabaseError(format!("Row error: {}", e)))?))
        } else {
            Ok(None)
        }
    }

    pub fn delete_show(&mut self, id: &str) -> Result<(), AppError> {
        // Enforce foreign keys so episodes are deleted via cascade
        self.conn
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| AppError::DatabaseError(format!("Failed to enable foreign keys: {}", e)))?;

        let rows = self
            .conn
            .execute("DELETE FROM shows WHERE id = ?1", params![id])
            .map_err(|e| AppError::DatabaseError(format!("Failed to delete show: {}", e)))?;

        if rows == 0 {
            return Err(AppError::NotFound(format!("Show {} not found", id)));
        }

        Ok(())
    }

    pub fn get_episode_by_source_path(
        &self,
        show_id: &str,
        source_path: &str,
    ) -> Result<Option<CatalogueEpisode>, AppError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, show_id, source_path, filename, file_size_bytes, duration_seconds,
                        format, codec, sample_rate, channels, bitrate, integrated_loudness_lufs,
                        true_peak_dbtp, leading_silence_seconds, trailing_silence_seconds,
                        clipping_evidence, overall_assessment_status, assessment_profile_id,
                        assessment_profile_version, analysed_at, source_modified_at, created_at,
                        updated_at, assessment_json
                 FROM episodes
                 WHERE show_id = ?1 AND source_path = ?2",
            )
            .map_err(|e| AppError::DatabaseError(format!("Failed to prepare get_episode_by_source_path: {}", e)))?;

        let mut rows = stmt
            .query_map(params![show_id, source_path], Self::map_episode_row)
            .map_err(|e| AppError::DatabaseError(format!("Failed to query episode: {}", e)))?;

        if let Some(r) = rows.next() {
            Ok(Some(r.map_err(|e| AppError::DatabaseError(format!("Row error: {}", e)))?))
        } else {
            Ok(None)
        }
    }

    pub fn get_episode_by_id(&self, id: &str) -> Result<Option<CatalogueEpisode>, AppError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, show_id, source_path, filename, file_size_bytes, duration_seconds,
                        format, codec, sample_rate, channels, bitrate, integrated_loudness_lufs,
                        true_peak_dbtp, leading_silence_seconds, trailing_silence_seconds,
                        clipping_evidence, overall_assessment_status, assessment_profile_id,
                        assessment_profile_version, analysed_at, source_modified_at, created_at,
                        updated_at, assessment_json
                 FROM episodes
                 WHERE id = ?1",
            )
            .map_err(|e| AppError::DatabaseError(format!("Failed to prepare get_episode_by_id: {}", e)))?;

        let mut rows = stmt
            .query_map(params![id], Self::map_episode_row)
            .map_err(|e| AppError::DatabaseError(format!("Failed to query episode: {}", e)))?;

        if let Some(r) = rows.next() {
            Ok(Some(r.map_err(|e| AppError::DatabaseError(format!("Row error: {}", e)))?))
        } else {
            Ok(None)
        }
    }

    pub fn insert_episode(&mut self, episode: &CatalogueEpisode) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT INTO episodes (
                id, show_id, source_path, filename, file_size_bytes, duration_seconds,
                format, codec, sample_rate, channels, bitrate, integrated_loudness_lufs,
                true_peak_dbtp, leading_silence_seconds, trailing_silence_seconds,
                clipping_evidence, overall_assessment_status, assessment_profile_id,
                assessment_profile_version, analysed_at, source_modified_at, created_at,
                updated_at, assessment_json
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24
            )",
            params![
                episode.id,
                episode.show_id,
                episode.source_path,
                episode.filename,
                episode.file_size_bytes,
                episode.duration_seconds,
                format!("{:?}", episode.format).to_uppercase(),
                episode.codec,
                episode.sample_rate,
                episode.channels,
                episode.bitrate,
                episode.integrated_loudness_lufs,
                episode.true_peak_dbtp,
                episode.leading_silence_seconds,
                episode.trailing_silence_seconds,
                episode.clipping_evidence,
                episode.overall_assessment_status,
                episode.assessment_profile_id,
                episode.assessment_profile_version,
                episode.analysed_at,
                episode.source_modified_at,
                episode.created_at,
                episode.updated_at,
                episode.assessment_json,
            ],
        ).map_err(|e| AppError::DatabaseError(format!("Failed to insert episode: {}", e)))?;

        // Update show updated_at
        let _ = self.conn.execute(
            "UPDATE shows SET updated_at = ?1 WHERE id = ?2",
            params![episode.updated_at, episode.show_id],
        );

        Ok(())
    }

    pub fn update_episode(&mut self, episode: &CatalogueEpisode) -> Result<(), AppError> {
        self.conn.execute(
            "UPDATE episodes SET
                filename = ?1,
                file_size_bytes = ?2,
                duration_seconds = ?3,
                format = ?4,
                codec = ?5,
                sample_rate = ?6,
                channels = ?7,
                bitrate = ?8,
                integrated_loudness_lufs = ?9,
                true_peak_dbtp = ?10,
                leading_silence_seconds = ?11,
                trailing_silence_seconds = ?12,
                clipping_evidence = ?13,
                overall_assessment_status = ?14,
                assessment_profile_id = ?15,
                assessment_profile_version = ?16,
                analysed_at = ?17,
                source_modified_at = ?18,
                updated_at = ?19,
                assessment_json = ?20
             WHERE id = ?21",
            params![
                episode.filename,
                episode.file_size_bytes,
                episode.duration_seconds,
                format!("{:?}", episode.format).to_uppercase(),
                episode.codec,
                episode.sample_rate,
                episode.channels,
                episode.bitrate,
                episode.integrated_loudness_lufs,
                episode.true_peak_dbtp,
                episode.leading_silence_seconds,
                episode.trailing_silence_seconds,
                episode.clipping_evidence,
                episode.overall_assessment_status,
                episode.assessment_profile_id,
                episode.assessment_profile_version,
                episode.analysed_at,
                episode.source_modified_at,
                episode.updated_at,
                episode.assessment_json,
                episode.id,
            ],
        ).map_err(|e| AppError::DatabaseError(format!("Failed to update episode: {}", e)))?;

        // Update show updated_at
        let _ = self.conn.execute(
            "UPDATE shows SET updated_at = ?1 WHERE id = ?2",
            params![episode.updated_at, episode.show_id],
        );

        Ok(())
    }

    pub fn get_episodes_for_show(&self, show_id: &str) -> Result<Vec<CatalogueEpisode>, AppError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, show_id, source_path, filename, file_size_bytes, duration_seconds,
                        format, codec, sample_rate, channels, bitrate, integrated_loudness_lufs,
                        true_peak_dbtp, leading_silence_seconds, trailing_silence_seconds,
                        clipping_evidence, overall_assessment_status, assessment_profile_id,
                        assessment_profile_version, analysed_at, source_modified_at, created_at,
                        updated_at, assessment_json
                 FROM episodes
                 WHERE show_id = ?1
                 ORDER BY analysed_at DESC",
            )
            .map_err(|e| AppError::DatabaseError(format!("Failed to prepare get_episodes_for_show: {}", e)))?;

        let rows = stmt
            .query_map(params![show_id], Self::map_episode_row)
            .map_err(|e| AppError::DatabaseError(format!("Failed to query episodes: {}", e)))?;

        let mut episodes = Vec::new();
        for r in rows {
            episodes.push(r.map_err(|e| AppError::DatabaseError(format!("Row error: {}", e)))?);
        }

        Ok(episodes)
    }

    pub fn delete_episode(&mut self, id: &str) -> Result<(), AppError> {
        let rows = self
            .conn
            .execute("DELETE FROM episodes WHERE id = ?1", params![id])
            .map_err(|e| AppError::DatabaseError(format!("Failed to delete episode: {}", e)))?;

        if rows == 0 {
            return Err(AppError::NotFound(format!("Episode {} not found", id)));
        }

        Ok(())
    }

    pub fn delete_episodes(&mut self, ids: &[String]) -> Result<usize, AppError> {
        if ids.is_empty() {
            return Ok(0);
        }

        let tx = self
            .conn
            .transaction()
            .map_err(|e| AppError::DatabaseError(format!("Failed to begin transaction: {}", e)))?;

        let mut deleted_count = 0;
        let mut affected_shows = std::collections::HashSet::new();

        for id in ids {
            // Find show_id first
            let show_id: Option<String> = tx
                .query_row(
                    "SELECT show_id FROM episodes WHERE id = ?1",
                    params![id],
                    |row| row.get(0),
                )
                .ok();

            if let Some(sid) = show_id {
                let affected = tx
                    .execute("DELETE FROM episodes WHERE id = ?1", params![id])
                    .map_err(|e| AppError::DatabaseError(format!("Failed to delete episode {}: {}", id, e)))?;
                if affected > 0 {
                    deleted_count += affected;
                    affected_shows.insert(sid);
                }
            }
        }

        let now = chrono::Utc::now().to_rfc3339();
        for sid in affected_shows {
            let _ = tx.execute(
                "UPDATE shows SET updated_at = ?1 WHERE id = ?2",
                params![now, sid],
            );
        }

        tx.commit()
            .map_err(|e| AppError::DatabaseError(format!("Failed to commit delete_episodes transaction: {}", e)))?;

        Ok(deleted_count)
    }

    pub fn move_episodes(
        &mut self,
        episode_ids: &[String],
        target_show_id: &str,
        now: &str,
    ) -> Result<MoveEpisodesResult, AppError> {
        // Verify target show exists
        let target_show: Show = self
            .get_show_by_id(target_show_id)?
            .ok_or_else(|| AppError::NotFound(format!("Target show {} not found", target_show_id)))?;

        let tx = self
            .conn
            .transaction()
            .map_err(|e| AppError::DatabaseError(format!("Failed to begin transaction for move_episodes: {}", e)))?;

        let mut moved = 0;
        let mut already_exists = 0;
        let mut failed = 0;
        let mut outcomes = Vec::new();
        let mut source_show_ids = std::collections::HashSet::new();

        for ep_id in episode_ids {
            // Fetch episode record
            let ep_info: Option<(String, String, String)> = tx
                .query_row(
                    "SELECT id, show_id, source_path, filename FROM episodes WHERE id = ?1",
                    params![ep_id],
                    |row| Ok((row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .ok();

            let (source_show_id, source_path, filename) = match ep_info {
                Some(info) => info,
                None => {
                    failed += 1;
                    outcomes.push(MoveEpisodeOutcome {
                        episode_id: ep_id.clone(),
                        filename: "Unknown".to_string(),
                        status: MoveEpisodeOutcomeStatus::Failed,
                        message: Some(format!("Episode {} not found", ep_id)),
                    });
                    continue;
                }
            };

            source_show_ids.insert(source_show_id.clone());

            // If source show is already target show
            if source_show_id == target_show_id {
                already_exists += 1;
                outcomes.push(MoveEpisodeOutcome {
                    episode_id: ep_id.clone(),
                    filename,
                    status: MoveEpisodeOutcomeStatus::AlreadyExists,
                    message: Some("Episode is already in the destination show.".to_string()),
                });
                continue;
            }

            // Check if target show already has an episode with this source_path
            let existing_in_target: Option<String> = tx
                .query_row(
                    "SELECT id FROM episodes WHERE show_id = ?1 AND source_path = ?2",
                    params![target_show_id, source_path],
                    |row| row.get(0),
                )
                .ok();

            if let Some(_target_ep_id) = existing_in_target {
                // Destination already contains this source file.
                // Destination record remains authoritative. Remove association from source show safely.
                tx.execute("DELETE FROM episodes WHERE id = ?1", params![ep_id])
                    .map_err(|e| AppError::DatabaseError(format!("Failed to clean up moved episode: {}", e)))?;

                already_exists += 1;
                outcomes.push(MoveEpisodeOutcome {
                    episode_id: ep_id.clone(),
                    filename,
                    status: MoveEpisodeOutcomeStatus::AlreadyExists,
                    message: Some(
                        "Episode with identical source already existed in destination show; source show association removed."
                            .to_string(),
                    ),
                });
            } else {
                // Normal move: update show_id and updated_at
                tx.execute(
                    "UPDATE episodes SET show_id = ?1, updated_at = ?2 WHERE id = ?3",
                    params![target_show_id, now, ep_id],
                )
                .map_err(|e| AppError::DatabaseError(format!("Failed to move episode {}: {}", ep_id, e)))?;

                moved += 1;
                outcomes.push(MoveEpisodeOutcome {
                    episode_id: ep_id.clone(),
                    filename,
                    status: MoveEpisodeOutcomeStatus::Moved,
                    message: Some("Episode moved to destination show.".to_string()),
                });
            }
        }

        // Update target show updated_at
        let _ = tx.execute(
            "UPDATE shows SET updated_at = ?1 WHERE id = ?2",
            params![now, target_show_id],
        );

        // Update source show(s) updated_at
        for sid in source_show_ids {
            let _ = tx.execute(
                "UPDATE shows SET updated_at = ?1 WHERE id = ?2",
                params![now, sid],
            );
        }

        tx.commit()
            .map_err(|e| AppError::DatabaseError(format!("Failed to commit move_episodes transaction: {}", e)))?;

        Ok(MoveEpisodesResult {
            target_show_id: target_show.id,
            target_show_name: target_show.name,
            total_requested: episode_ids.len(),
            moved,
            already_exists,
            failed,
            outcomes,
        })
    }

    #[allow(dead_code)]
    pub fn relink_episode(
        &mut self,
        episode_id: &str,
        new_source_path: &str,
        new_filename: &str,
        file_size_bytes: i64,
        source_modified_at: Option<&str>,
        now: &str,
    ) -> Result<(), AppError> {
        let rows = self
            .conn
            .execute(
                "UPDATE episodes SET
                    source_path = ?1,
                    filename = ?2,
                    file_size_bytes = ?3,
                    source_modified_at = ?4,
                    updated_at = ?5
                 WHERE id = ?6",
                params![
                    new_source_path,
                    new_filename,
                    file_size_bytes,
                    source_modified_at,
                    now,
                    episode_id,
                ],
            )
            .map_err(|e| AppError::DatabaseError(format!("Failed to relink episode: {}", e)))?;

        if rows == 0 {
            return Err(AppError::NotFound(format!("Episode {} not found", episode_id)));
        }

        // Update show updated_at
        let show_id: Option<String> = self
            .conn
            .query_row(
                "SELECT show_id FROM episodes WHERE id = ?1",
                params![episode_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(sid) = show_id {
            let _ = self.conn.execute(
                "UPDATE shows SET updated_at = ?1 WHERE id = ?2",
                params![now, sid],
            );
        }

        Ok(())
    }

    fn map_episode_row(row: &Row) -> Result<CatalogueEpisode, rusqlite::Error> {
        let format_str: String = row.get(6)?;
        let format = match format_str.as_str() {
            "WAV" => MediaFormat::WAV,
            "MP3" => MediaFormat::MP3,
            "M4A" => MediaFormat::M4A,
            "MOV" => MediaFormat::MOV,
            "MP4" => MediaFormat::MP4,
            _ => MediaFormat::UNKNOWN,
        };


        let assessment_json: Option<String> = row.get(23)?;
        let assessment: Option<Assessment> = assessment_json
            .as_ref()
            .and_then(|j| serde_json::from_str(j).ok());

        let source_path: String = row.get(2)?;
        let file_size_bytes: i64 = row.get(4)?;
        let source_modified_at: Option<String> = row.get(20)?;

        let source_path_buf = std::path::Path::new(&source_path);
        let source_availability = if !source_path_buf.exists() {
            SourceAvailability::Missing
        } else if let Ok(meta) = std::fs::metadata(source_path_buf) {
            let current_size = meta.len() as i64;
            let current_mtime = meta
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());

            let size_matches = current_size == file_size_bytes;
            let mtime_matches = match (&source_modified_at, &current_mtime) {
                (Some(a), Some(b)) => a == b,
                (None, None) => true,
                _ => false,
            };

            if size_matches && mtime_matches {
                SourceAvailability::Available
            } else {
                SourceAvailability::Changed
            }
        } else {
            SourceAvailability::Missing
        };


        let raw_status: String = row.get(16)?;
        let overall_assessment_status = match raw_status.as_str() {
            "NEEDSATTENTION" | "NEEDS_ATTENTION" => "NEEDS_ATTENTION".to_string(),
            "ATTENTION" => "ATTENTION".to_string(),
            "READY" => "READY".to_string(),
            other => other.to_string(),
        };

        Ok(CatalogueEpisode {
            id: row.get(0)?,
            show_id: row.get(1)?,
            source_path,
            filename: row.get(3)?,
            file_size_bytes: row.get(4)?,
            duration_seconds: row.get(5)?,
            format,
            codec: row.get(7)?,
            sample_rate: row.get(8)?,
            channels: row.get(9)?,
            bitrate: row.get(10)?,
            integrated_loudness_lufs: row.get(11)?,
            true_peak_dbtp: row.get(12)?,
            leading_silence_seconds: row.get(13)?,
            trailing_silence_seconds: row.get(14)?,
            clipping_evidence: row.get(15)?,
            overall_assessment_status,
            assessment_profile_id: row.get(17)?,
            assessment_profile_version: row.get(18)?,
            analysed_at: row.get(19)?,
            source_modified_at: row.get(20)?,
            created_at: row.get(21)?,
            updated_at: row.get(22)?,
            assessment_json,
            assessment,
            source_availability,
        })
    }
}
