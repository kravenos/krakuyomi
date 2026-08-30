use std::collections::HashMap;

use axum::extract::{Path, Query, State as StateExtractor};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use shared::model::{InstallOutcome, SourceId};
use shared::resource_usage::SourceUsage;
use shared::settings::SourceSettingValue;
use shared::source::model::SettingDefinition;
use shared::source::SourceBackend;
use shared::usecases;

use crate::model::SourceInformation;
use crate::source_extractor::{SourceExtractor, SourceParams};
use crate::state::State;
use crate::AppError;

pub fn routes() -> Router<State> {
    Router::new()
        .route("/available-sources", get(list_available_sources))
        .route("/source-catalogs/refresh", post(list_available_sources))
        .route(
            "/available-sources/{source_id}/install",
            post(install_source),
        )
        .route("/installed-sources", get(list_installed_sources))
        .route("/sources/status", get(list_source_statuses))
        .route(
            "/installed-sources/{source_id}/uninstall-preview",
            get(get_uninstall_preview),
        )
        .route("/installed-sources/{source_id}", delete(uninstall_source))
        .route(
            "/installed-sources/{source_id}/setting-definitions",
            get(get_source_setting_definitions),
        )
        .route(
            "/installed-sources/{source_id}/stored-settings",
            get(get_source_stored_settings),
        )
        .route(
            "/installed-sources/{source_id}/stored-settings",
            post(set_source_stored_settings),
        )
        .route("/installed-sources/usage", get(get_all_source_usage))
}

async fn list_available_sources(
    StateExtractor(State {
        settings,
        catalog_cache_path,
        ..
    }): StateExtractor<State>,
) -> Result<Json<Vec<SourceInformation>>, AppError> {
    let source_lists = settings.lock().await.source_lists.clone();
    let available_sources = usecases::list_available_sources(&source_lists, catalog_cache_path)
        .await?
        .into_iter()
        .map(SourceInformation::from)
        .collect();

    Ok(Json(available_sources))
}

#[derive(Deserialize)]
struct InstallSourceParams {
    source_id: String,
}

#[derive(Deserialize)]
struct InstallSourceRequest {
    list_id: String,
    version: serde_json::Value,
    #[serde(default)]
    languages: Option<Vec<String>>,
}

async fn install_source(
    StateExtractor(State {
        source_manager,
        settings,
        catalog_cache_path,
        ..
    }): StateExtractor<State>,
    Path(InstallSourceParams { source_id }): Path<InstallSourceParams>,
    Json(InstallSourceRequest {
        list_id,
        version,
        languages,
    }): Json<InstallSourceRequest>,
) -> Result<Json<InstallOutcome>, AppError> {
    let source_lists = settings.lock().await.source_lists.clone();
    // The use case locks the manager inside a blocking task: the eager probe
    // (JS evaluation / runtime boot) must not run on the async worker.
    let outcome = usecases::install_source(
        &source_manager,
        &source_lists,
        &catalog_cache_path,
        SourceId::new(source_id),
        list_id,
        version,
        languages,
    )
    .await?;

    Ok(Json(outcome))
}

async fn list_installed_sources(
    StateExtractor(State { source_manager, .. }): StateExtractor<State>,
) -> Json<Vec<SourceInformation>> {
    let installed_sources = usecases::list_installed_sources(&*source_manager.lock().await)
        .into_iter()
        .map(SourceInformation::from)
        .collect();

    Json(installed_sources)
}

#[derive(serde::Serialize)]
struct SourcePackageStatusResponse {
    source_ids: Vec<String>,
    package_label: String,
    package_kind: &'static str,
    presence: &'static str,
    load: &'static str,
    catalog: &'static str,
    freshness: &'static str,
    runtime: &'static str,
    compatibility: &'static str,
    error: Option<String>,
}

async fn list_source_statuses(
    StateExtractor(State { source_manager, .. }): StateExtractor<State>,
) -> Json<Vec<SourcePackageStatusResponse>> {
    let source_manager = source_manager.lock().await;
    Json(
        source_manager
            .source_packages
            .iter()
            .map(|status| SourcePackageStatusResponse {
                source_ids: status
                    .source_ids
                    .iter()
                    .map(|id| id.value().clone())
                    .collect(),
                package_label: status.package_label.clone(),
                package_kind: status.kind.as_str(),
                presence: "installed",
                load: status.load.as_str(),
                catalog: "unknown",
                freshness: "unknown",
                runtime: "unknown",
                compatibility: "unknown",
                error: status.error.clone(),
            })
            .collect(),
    )
}

#[derive(serde::Serialize)]
struct SourceUninstallPreview {
    source_ids: Vec<String>,
    library_manga_count: usize,
}

async fn get_uninstall_preview(
    StateExtractor(State {
        source_manager,
        database,
        ..
    }): StateExtractor<State>,
    Path(SourceParams { source_id }): Path<SourceParams>,
) -> Result<Json<SourceUninstallPreview>, AppError> {
    let source_ids = source_manager
        .lock()
        .await
        .package_source_ids(&SourceId::new(source_id))?;
    let library_manga_count = database
        .count_library_mangas_for_sources(&source_ids)
        .await?;

    Ok(Json(SourceUninstallPreview {
        source_ids: source_ids
            .into_iter()
            .map(|source_id| source_id.value().clone())
            .collect(),
        library_manga_count,
    }))
}

#[derive(Deserialize)]
struct UninstallSourceQuery {
    confirmed_library_count: usize,
}

async fn uninstall_source(
    StateExtractor(State {
        source_manager,
        database,
        ..
    }): StateExtractor<State>,
    Path(SourceParams { source_id }): Path<SourceParams>,
    Query(UninstallSourceQuery {
        confirmed_library_count,
    }): Query<UninstallSourceQuery>,
) -> Result<Json<()>, AppError> {
    let source_id = SourceId::new(source_id);
    let mut source_manager = source_manager.lock().await;
    let affected_source_ids = source_manager.package_source_ids(&source_id)?;
    let current_library_count = database
        .count_library_mangas_for_sources(&affected_source_ids)
        .await?;
    verify_uninstall_count(confirmed_library_count, current_library_count)?;

    usecases::uninstall_source(&mut source_manager, source_id)?;

    Ok(Json(()))
}

fn verify_uninstall_count(confirmed: usize, current: usize) -> Result<(), AppError> {
    if current == confirmed {
        return Ok(());
    }

    Err(AppError::Conflict(format!(
        "The library changed after this uninstall was reviewed (expected {confirmed}, now {current}). Review the source again before removing it."
    )))
}

async fn get_source_setting_definitions(
    SourceExtractor(source): SourceExtractor,
) -> Result<Json<Vec<SettingDefinition>>, AppError> {
    // LNReader/MangaYomi sources probe lazily on first use; the probe
    // evaluates JS / boots a runtime, so run it off the async worker and
    // propagate its outcome instead of silently returning no definitions.
    let definitions = tokio::task::spawn_blocking(move || {
        source.probe()?;
        Ok::<_, anyhow::Error>(usecases::get_source_setting_definitions(&source))
    })
    .await
    .map_err(|e| AppError::Other(anyhow::anyhow!("settings probe task failed: {e}")))?
    .map_err(AppError::Other)?;

    Ok(Json(definitions))
}

async fn get_source_stored_settings(
    StateExtractor(State { settings, .. }): StateExtractor<State>,
    Path(SourceParams { source_id }): Path<SourceParams>,
) -> Json<HashMap<String, SourceSettingValue>> {
    Json(usecases::get_source_stored_settings(
        &*settings.lock().await,
        &SourceId::new(source_id),
    ))
}

async fn set_source_stored_settings(
    StateExtractor(State {
        settings,
        settings_path,
        source_manager,
        ..
    }): StateExtractor<State>,
    Path(SourceParams { source_id }): Path<SourceParams>,
    Json(stored_settings): Json<HashMap<String, SourceSettingValue>>,
) -> Result<Json<()>, AppError> {
    usecases::set_source_stored_settings(
        &mut *settings.lock().await,
        &settings_path,
        &mut *source_manager.lock().await,
        &source_manager,
        &SourceId::new(source_id),
        stored_settings,
    )?;

    Ok(Json(()))
}

#[derive(serde::Serialize)]
struct SourceUsageResponse {
    #[serde(flatten)]
    usage: SourceUsage,
    disk_bytes: u64,
}

fn file_size(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// The on-disk files backing a source, without touching the filesystem.
fn source_files_of(
    source_manager: &shared::source_manager::SourceManager,
    source: &shared::source::Source,
) -> Vec<std::path::PathBuf> {
    let source_id = SourceId::new(source.manifest().info.id.clone());
    match &source.backend {
        SourceBackend::Aidoku(_) => vec![source_manager.source_path(&source_id)],
        SourceBackend::LnReader(_) => vec![
            source_manager.lnreader_source_path(&source_id),
            source_manager.lnreader_probe_path(&source_id),
        ],
        SourceBackend::Mangayomi(_) => vec![
            source_manager.mangayomi_source_path(&source_id),
            source_manager.mangayomi_js_source_path(&source_id),
            source_manager.mangayomi_probe_path(&source_id),
        ],
        SourceBackend::Keiyoushi(_) => vec![
            source_manager.keiyoushi_source_path(&source_id),
            source_manager.keiyoushi_probe_path(&source_id),
        ],
    }
}

fn disk_bytes_of(files: &[std::path::PathBuf]) -> u64 {
    files.iter().map(|p| file_size(p)).sum()
}

/// Returns the runtime usage of every installed source in one response,
/// keyed by source id. Polling this endpoint keeps the demand-driven VM
/// memory tracking alive (see [`ResourceRegistry::mark_active`]).
async fn get_all_source_usage(
    StateExtractor(State { source_manager, .. }): StateExtractor<State>,
) -> Json<HashMap<String, SourceUsageResponse>> {
    let source_manager = source_manager.lock().await;
    let entries = source_manager
        .sources_by_id
        .iter()
        .map(|(source_id, source)| {
            // Read first: the first poll of a reopened view discards any
            // stale memory data left from the previous session, then the
            // poll itself restarts the demand-driven tracking.
            let usage = source.usage.usage(source_id.value()).unwrap_or_default();
            source.usage.mark_active();
            let files = source_files_of(&source_manager, source);
            (source_id.value().to_string(), usage, files)
        })
        .collect::<Vec<_>>();
    drop(source_manager);

    let out = tokio::task::spawn_blocking(move || {
        entries
            .into_iter()
            .map(|(source_id, usage, files)| {
                (
                    source_id,
                    SourceUsageResponse {
                        usage,
                        disk_bytes: disk_bytes_of(&files),
                    },
                )
            })
            .collect::<HashMap<_, _>>()
    })
    .await
    .unwrap_or_default();

    Json(out)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;

    #[test]
    fn stale_uninstall_preview_is_rejected_as_a_conflict() {
        let error = verify_uninstall_count(18, 19).expect_err("stale preview must fail");

        assert_eq!(StatusCode::from(&error), StatusCode::CONFLICT);
        assert!(matches!(error, AppError::Conflict(_)));
    }

    #[test]
    fn unchanged_uninstall_preview_is_accepted() {
        assert!(verify_uninstall_count(18, 18).is_ok());
    }
}
