use std::collections::{BTreeSet, HashMap};

use axum::extract::{Path, Query, State as StateExtractor};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use shared::model::{InstallOutcome, SourceId};
use shared::resource_usage::SourceUsage;
use shared::settings::{SourceList, SourceListType, SourceSettingValue};
use shared::source::model::SettingDefinition;
use shared::source::SourceBackend;
use shared::source_catalog::{
    is_newer_source_version, source_list_id, CatalogCandidate, CatalogListSummary, CatalogStore,
};
use shared::source_health::{SourceHealthSummary, SourceOperationClass, SourceRuntimeHealth};
use shared::source_manager::{SourcePackageKind, SourcePackageLoadState, SourcePackageStatus};
use shared::usecases;

use crate::model::SourceInformation;
use crate::source_extractor::{SourceExtractor, SourceParams};
use crate::state::State;
use crate::AppError;

pub fn routes() -> Router<State> {
    Router::new()
        .route("/available-sources", get(list_available_sources))
        .route("/source-catalogs/refresh", post(list_available_sources))
        .route("/source-catalogs/status", get(list_source_catalogs))
        .route("/source-catalogs/validate", post(validate_source_catalog))
        .route(
            "/source-catalogs/{list_id}/refresh",
            post(refresh_source_catalog),
        )
        .route(
            "/source-catalogs/{list_id}/change-preview",
            get(get_source_catalog_change_preview),
        )
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

async fn list_source_catalogs(
    StateExtractor(State {
        settings,
        catalog_cache_path,
        ..
    }): StateExtractor<State>,
) -> Result<Json<Vec<CatalogListSummary>>, AppError> {
    let source_lists = settings.lock().await.source_lists.clone();
    let summaries = tokio::task::spawn_blocking(move || {
        CatalogStore::new(catalog_cache_path).summaries(&source_lists)
    })
    .await
    .map_err(|error| AppError::SourceStatus(error.into()))?;
    Ok(Json(summaries))
}

async fn validate_source_catalog(
    Json(source_list): Json<SourceList>,
) -> Result<Json<CatalogListSummary>, AppError> {
    CatalogStore::validate(&source_list)
        .await
        .map(Json)
        .map_err(AppError::SourceCatalog)
}

#[derive(Deserialize)]
struct SourceCatalogParams {
    list_id: String,
}

async fn refresh_source_catalog(
    StateExtractor(State {
        settings,
        catalog_cache_path,
        ..
    }): StateExtractor<State>,
    Path(SourceCatalogParams { list_id }): Path<SourceCatalogParams>,
) -> Result<Json<CatalogListSummary>, AppError> {
    let source_lists = settings.lock().await.source_lists.clone();
    let (configured_order, source_list) = source_lists
        .into_iter()
        .enumerate()
        .find(|(_, source_list)| source_list_id(source_list) == list_id)
        .ok_or(AppError::NotFound)?;
    CatalogStore::new(catalog_cache_path)
        .refresh_one(&source_list, configured_order)
        .await
        .map(Json)
        .map_err(AppError::SourceCatalog)
}

#[derive(Clone, Debug, serde::Serialize)]
struct CatalogCoverageSource {
    source_id: String,
    name: String,
    presence: &'static str,
    library_manga_count: usize,
}

#[derive(serde::Serialize)]
struct SourceCatalogChangePreview {
    list_id: String,
    coverage_known: bool,
    affected_sources: Vec<CatalogCoverageSource>,
}

async fn get_source_catalog_change_preview(
    StateExtractor(State {
        settings,
        catalog_cache_path,
        source_manager,
        database,
        ..
    }): StateExtractor<State>,
    Path(SourceCatalogParams { list_id }): Path<SourceCatalogParams>,
) -> Result<Json<SourceCatalogChangePreview>, AppError> {
    let source_lists = settings.lock().await.source_lists.clone();
    if !source_lists
        .iter()
        .any(|source_list| source_list_id(source_list) == list_id)
    {
        return Err(AppError::NotFound);
    }
    let active_list_ids = source_lists
        .iter()
        .filter(|source_list| source_list.enabled)
        .map(source_list_id)
        .collect::<BTreeSet<_>>();
    let candidates = tokio::task::spawn_blocking(move || {
        CatalogStore::new(catalog_cache_path).load_all(&source_lists)
    })
    .await
    .map_err(|error| AppError::SourceStatus(error.into()))?
    .map_err(AppError::SourceStatus)?;
    let (installed, packages) = {
        let source_manager = source_manager.lock().await;
        (
            usecases::list_installed_sources(&source_manager),
            source_manager.source_packages.clone(),
        )
    };
    let library_counts = database
        .count_library_mangas_by_source()
        .await
        .map_err(|error| AppError::SourceStatus(error.into()))?;

    Ok(Json(build_catalog_change_preview(
        list_id,
        active_list_ids,
        candidates,
        installed,
        packages,
        library_counts,
    )))
}

fn build_catalog_change_preview(
    list_id: String,
    active_list_ids: BTreeSet<String>,
    candidates: Vec<CatalogCandidate>,
    installed: Vec<shared::model::SourceInformation>,
    packages: Vec<SourcePackageStatus>,
    library_counts: HashMap<String, usize>,
) -> SourceCatalogChangePreview {
    let target_is_active = active_list_ids.contains(&list_id);
    let target = candidates
        .iter()
        .filter(|candidate| target_is_active && candidate.list_id == list_id)
        .collect::<Vec<_>>();
    let coverage_known = !target_is_active || !target.is_empty();
    let remaining = candidates
        .iter()
        .filter(|candidate| {
            candidate.list_id != list_id && active_list_ids.contains(&candidate.list_id)
        })
        .collect::<Vec<_>>();
    let installed_names = installed
        .iter()
        .map(|source| (source.id.value().clone(), source.name.clone()))
        .collect::<HashMap<_, _>>();
    let mut installed_kinds = HashMap::new();
    for package in &packages {
        for source_id in &package.source_ids {
            installed_kinds.insert(source_id.value().clone(), Some(package.kind));
        }
    }
    for source in &installed {
        installed_kinds
            .entry(source.id.value().clone())
            .or_insert(None);
    }

    let mut affected = HashMap::<String, CatalogCoverageSource>::new();
    for (source_id, package_kind) in &installed_kinds {
        let supplied_by_target = target.iter().any(|candidate| {
            candidate.source.id.value() == source_id
                && package_kind.is_none_or(|kind| source_type_matches(kind, candidate.source_type))
        });
        let supplied_elsewhere = remaining.iter().any(|candidate| {
            candidate.source.id.value() == source_id
                && package_kind.is_none_or(|kind| source_type_matches(kind, candidate.source_type))
        });
        if supplied_by_target && !supplied_elsewhere {
            let name = installed_names
                .get(source_id)
                .cloned()
                .or_else(|| {
                    target
                        .iter()
                        .find(|candidate| candidate.source.id.value() == source_id)
                        .map(|candidate| candidate.source.name.clone())
                })
                .unwrap_or_else(|| source_id.clone());
            affected.insert(
                source_id.clone(),
                CatalogCoverageSource {
                    source_id: source_id.clone(),
                    name,
                    presence: "installed",
                    library_manga_count: library_counts.get(source_id).copied().unwrap_or(0),
                },
            );
        }
    }

    for (source_id, library_manga_count) in library_counts {
        if installed_kinds.contains_key(&source_id) {
            continue;
        }
        let supplied_by_target = target
            .iter()
            .any(|candidate| candidate.source.id.value() == &source_id);
        let supplied_elsewhere = remaining
            .iter()
            .any(|candidate| candidate.source.id.value() == &source_id);
        if supplied_by_target && !supplied_elsewhere {
            let name = target
                .iter()
                .find(|candidate| candidate.source.id.value() == &source_id)
                .map(|candidate| candidate.source.name.clone())
                .unwrap_or_else(|| source_id.clone());
            affected.insert(
                source_id.clone(),
                CatalogCoverageSource {
                    source_id,
                    name,
                    presence: "missing",
                    library_manga_count,
                },
            );
        }
    }

    let mut affected_sources = affected.into_values().collect::<Vec<_>>();
    affected_sources.sort_by(|left, right| {
        left.presence
            .cmp(right.presence)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    SourceCatalogChangePreview {
        list_id,
        coverage_known,
        affected_sources,
    }
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

#[derive(Clone, serde::Serialize)]
struct SourceStatusResponse {
    source_id: String,
    name: String,
    languages: Vec<String>,
    library_manga_count: usize,
    installed_version: Option<serde_json::Value>,
    available_version: Option<serde_json::Value>,
    package_label: Option<String>,
    package_kind: Option<&'static str>,
    presence: &'static str,
    load: &'static str,
    catalog: &'static str,
    freshness: &'static str,
    runtime: &'static str,
    compatibility: &'static str,
    installed_list_id: Option<String>,
    selected_list_id: Option<String>,
    installed_provider_url: Option<String>,
    available_provider_url: Option<String>,
    catalog_fetched_at: Option<String>,
    catalog_age_seconds: Option<i64>,
    catalog_last_fetch_error: Option<String>,
    health_sample_count: usize,
    latest_operation: Option<&'static str>,
    latest_operation_at: Option<String>,
    latest_category: Option<String>,
    latest_message: Option<String>,
    error: Option<String>,
}

async fn list_source_statuses(
    StateExtractor(State {
        source_manager,
        database,
        settings,
        catalog_cache_path,
        source_health,
        ..
    }): StateExtractor<State>,
) -> Result<Json<Vec<SourceStatusResponse>>, AppError> {
    let (installed, packages) = {
        let source_manager = source_manager.lock().await;
        (
            usecases::list_installed_sources(&source_manager),
            source_manager.source_packages.clone(),
        )
    };
    let source_lists = settings.lock().await.source_lists.clone();
    let library_counts = database
        .count_library_mangas_by_source()
        .await
        .map_err(|error| AppError::SourceStatus(error.into()))?;
    let health = source_health.summaries().await;
    let catalog_task = tokio::task::spawn_blocking(move || {
        CatalogStore::new(catalog_cache_path).load(&source_lists)
    });
    let candidates = match catalog_task.await {
        Ok(Ok(candidates)) => candidates,
        Ok(Err(error)) => {
            log::warn!("couldn't load cached source catalogs for status: {error:#}");
            Vec::new()
        }
        Err(error) => {
            log::warn!("source catalog status task failed: {error}");
            Vec::new()
        }
    };

    Ok(Json(build_source_statuses(
        installed,
        packages,
        library_counts,
        candidates,
        health,
    )))
}

fn build_source_statuses(
    installed: Vec<shared::model::SourceInformation>,
    packages: Vec<SourcePackageStatus>,
    library_counts: HashMap<String, usize>,
    candidates: Vec<CatalogCandidate>,
    health: Vec<SourceHealthSummary>,
) -> Vec<SourceStatusResponse> {
    let installed = installed
        .into_iter()
        .map(|source| (source.id.value().clone(), source))
        .collect::<HashMap<_, _>>();
    let health = health
        .into_iter()
        .map(|summary| (summary.source_id.clone(), summary))
        .collect::<HashMap<_, _>>();
    let catalog_has_candidates = !candidates.is_empty();
    let selected_candidates = candidates
        .iter()
        .filter(|candidate| candidate.selected)
        .collect::<Vec<_>>();
    let mut represented_loaded = BTreeSet::new();
    let mut represented_ids = BTreeSet::new();
    let mut statuses = Vec::new();

    for package in &packages {
        for source_id in &package.source_ids {
            let source_id = source_id.value();
            let loaded = if package.load == SourcePackageLoadState::Loaded {
                installed.get(source_id)
            } else {
                None
            };
            if loaded.is_some() {
                represented_loaded.insert(source_id.clone());
            }
            represented_ids.insert(source_id.clone());
            statuses.push(build_source_status(
                source_id,
                loaded,
                Some(package),
                library_counts.get(source_id).copied().unwrap_or(0),
                choose_candidate(source_id, Some(package.kind), &selected_candidates),
                health.get(source_id),
                catalog_has_candidates,
                catalog_contains_source(source_id, Some(package.kind), &selected_candidates),
            ));
        }
    }

    for (source_id, loaded) in &installed {
        if represented_loaded.insert(source_id.clone()) {
            represented_ids.insert(source_id.clone());
            statuses.push(build_source_status(
                source_id,
                Some(loaded),
                None,
                library_counts.get(source_id).copied().unwrap_or(0),
                choose_candidate(source_id, None, &selected_candidates),
                health.get(source_id),
                catalog_has_candidates,
                catalog_contains_source(source_id, None, &selected_candidates),
            ));
        }
    }

    for (source_id, library_manga_count) in &library_counts {
        if represented_ids.insert(source_id.clone()) {
            statuses.push(build_source_status(
                source_id,
                None,
                None,
                *library_manga_count,
                choose_candidate(source_id, None, &selected_candidates),
                health.get(source_id),
                catalog_has_candidates,
                catalog_contains_source(source_id, None, &selected_candidates),
            ));
        }
    }

    statuses.sort_by(|left, right| {
        source_problem_rank(left)
            .cmp(&source_problem_rank(right))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.source_id.cmp(&right.source_id))
            .then_with(|| left.package_label.cmp(&right.package_label))
    });
    statuses
}

fn build_source_status(
    source_id: &str,
    installed: Option<&shared::model::SourceInformation>,
    package: Option<&SourcePackageStatus>,
    library_manga_count: usize,
    candidate: Option<&CatalogCandidate>,
    health: Option<&SourceHealthSummary>,
    catalog_has_candidates: bool,
    catalog_contains_source: bool,
) -> SourceStatusResponse {
    let installed_version = installed.map(|source| {
        source
            .installed_version
            .clone()
            .unwrap_or_else(|| source.version.clone())
    });
    let available_version = candidate.map(|candidate| candidate.source.version.clone());
    let freshness = match (&installed_version, candidate) {
        (Some(installed_version), Some(candidate))
            if is_newer_source_version(
                candidate.source_type,
                &candidate.source.version,
                installed_version,
            ) =>
        {
            "update_available"
        }
        (Some(_), Some(_)) => "current",
        _ => "unknown",
    };
    let loaded = package.is_some_and(|package| package.load == SourcePackageLoadState::Loaded)
        || installed.is_some();
    let runtime = health
        .map(|summary| runtime_key(summary.runtime))
        .unwrap_or("unknown");
    let catalog_fetched_at = candidate.map(|candidate| candidate.fetched_at.to_rfc3339());
    let catalog_age_seconds = candidate.map(|candidate| {
        chrono::Utc::now()
            .signed_duration_since(candidate.fetched_at)
            .num_seconds()
            .max(0)
    });
    let latest_message = health.and_then(|summary| summary.latest_message.clone());
    let error = package
        .and_then(|package| package.error.clone())
        .or_else(|| {
            (runtime == "failing")
                .then(|| latest_message.clone())
                .flatten()
        });

    SourceStatusResponse {
        source_id: source_id.to_owned(),
        name: installed
            .map(|source| source.name.clone())
            .or_else(|| candidate.map(|candidate| candidate.source.name.clone()))
            .unwrap_or_else(|| format!("Missing source: {source_id}")),
        languages: installed
            .map(|source| source.languages.clone())
            .or_else(|| candidate.map(|candidate| candidate.source.languages.clone()))
            .unwrap_or_default(),
        library_manga_count,
        installed_version,
        available_version,
        package_label: package.map(|package| package.package_label.clone()),
        package_kind: package.map(|package| package.kind.as_str()),
        presence: if package.is_some() || installed.is_some() {
            "installed"
        } else {
            "missing"
        },
        load: package
            .map(|package| package.load.as_str())
            .unwrap_or(if installed.is_some() {
                "loaded"
            } else {
                "not_applicable"
            }),
        catalog: if candidate.is_some() || catalog_contains_source {
            "available"
        } else if catalog_has_candidates {
            "unavailable"
        } else {
            "unknown"
        },
        freshness,
        runtime,
        compatibility: if loaded || candidate.is_some() {
            "compatible"
        } else {
            "unknown"
        },
        installed_list_id: installed.and_then(|source| source.catalog_list_id.clone()),
        selected_list_id: candidate.map(|candidate| candidate.list_id.clone()),
        installed_provider_url: installed
            .and_then(|source| source.provider_url.clone())
            .map(crate::model::safe_url_for_display),
        available_provider_url: candidate
            .map(|candidate| crate::model::safe_url_for_display(candidate.provider_url.clone())),
        catalog_fetched_at,
        catalog_age_seconds,
        catalog_last_fetch_error: candidate.and_then(|candidate| {
            candidate.last_fetch_error.as_ref().map(|_| {
                "The source list could not be refreshed; cached data is being used.".to_owned()
            })
        }),
        health_sample_count: health.map(|summary| summary.sample_count).unwrap_or(0),
        latest_operation: health.and_then(|summary| summary.latest_operation.map(operation_key)),
        latest_operation_at: health
            .and_then(|summary| summary.latest_at)
            .map(|timestamp| timestamp.to_rfc3339()),
        latest_category: health
            .and_then(|summary| summary.latest_category)
            .map(|category| category.code().to_owned()),
        latest_message,
        error,
    }
}

fn catalog_contains_source(
    source_id: &str,
    package_kind: Option<SourcePackageKind>,
    candidates: &[&CatalogCandidate],
) -> bool {
    candidates.iter().any(|candidate| {
        candidate.source.id.value() == source_id
            && package_kind.is_none_or(|kind| source_type_matches(kind, candidate.source_type))
    })
}

fn choose_candidate<'a>(
    source_id: &str,
    package_kind: Option<SourcePackageKind>,
    candidates: &[&'a CatalogCandidate],
) -> Option<&'a CatalogCandidate> {
    let mut matches = candidates.iter().copied().filter(|candidate| {
        candidate.source.id.value() == source_id
            && package_kind.is_none_or(|kind| source_type_matches(kind, candidate.source_type))
    });
    let candidate = matches.next()?;
    // A missing source id alone cannot distinguish two package formats.
    // Withhold the action instead of guessing which package to install.
    if package_kind.is_none() && matches.next().is_some() {
        None
    } else {
        Some(candidate)
    }
}

fn source_type_matches(package_kind: SourcePackageKind, source_type: SourceListType) -> bool {
    matches!(
        (package_kind, source_type),
        (SourcePackageKind::Aidoku, SourceListType::Aidoku)
            | (SourcePackageKind::LnReader, SourceListType::LnReader)
            | (SourcePackageKind::MangaYomi, SourceListType::Mangayomi)
            | (SourcePackageKind::Keiyoushi, SourceListType::Keiyoushi)
    )
}

fn runtime_key(runtime: SourceRuntimeHealth) -> &'static str {
    match runtime {
        SourceRuntimeHealth::Healthy => "healthy",
        SourceRuntimeHealth::Failing => "failing",
        SourceRuntimeHealth::Unknown => "unknown",
    }
}

fn operation_key(operation: SourceOperationClass) -> &'static str {
    match operation {
        SourceOperationClass::Search => "search",
        SourceOperationClass::RefreshChapters => "refresh_chapters",
        SourceOperationClass::RefreshDetails => "refresh_details",
    }
}

fn source_problem_rank(status: &SourceStatusResponse) -> u8 {
    if status.presence == "missing" {
        0
    } else if status.load == "load_failed" || status.compatibility == "incompatible" {
        1
    } else if status.runtime == "failing" {
        2
    } else if status.freshness == "update_available" {
        3
    } else if status.catalog == "unavailable" {
        4
    } else {
        5
    }
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
    use chrono::{Duration, Utc};
    use serde_json::json;
    use shared::source_health::SourceErrorCategory;
    use url::Url;

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

    fn source(
        id: &str,
        name: &str,
        version: serde_json::Value,
    ) -> shared::model::SourceInformation {
        shared::model::SourceInformation {
            id: SourceId::new(id.to_owned()),
            name: name.to_owned(),
            version,
            languages: vec!["en".to_owned()],
            source_of_source: Some("example".to_owned()),
            missing: false,
            catalog_list_id: Some("list-installed".to_owned()),
            provider_url: Some(Url::parse("https://example.com/catalog.json").unwrap()),
            resolved_provider_url: None,
            installed_version: None,
        }
    }

    fn candidate(id: &str, name: &str, version: serde_json::Value) -> CatalogCandidate {
        CatalogCandidate {
            list_id: "list-selected".to_owned(),
            source_type: SourceListType::LnReader,
            provider_url: Url::parse("https://example.com/catalog.json").unwrap(),
            resolved_provider_url: Url::parse("https://cdn.example.com/catalog.json").unwrap(),
            list_order: 0,
            fetched_at: Utc::now() - Duration::minutes(5),
            last_fetch_error: None,
            source: source(id, name, version),
            raw: json!({"id": id}),
            selected: true,
        }
    }

    #[test]
    fn source_statuses_put_problems_first_and_keep_actionable_details() {
        let installed = vec![source("update.source", "Update Source", json!("1.0.0"))];
        let packages = vec![
            SourcePackageStatus {
                package_label: "update.source.lnreader.js".to_owned(),
                kind: SourcePackageKind::LnReader,
                source_ids: vec![SourceId::new("update.source".to_owned())],
                load: SourcePackageLoadState::Loaded,
                error: None,
            },
            SourcePackageStatus {
                package_label: "broken.source.lnreader.js".to_owned(),
                kind: SourcePackageKind::LnReader,
                source_ids: vec![SourceId::new("broken.source".to_owned())],
                load: SourcePackageLoadState::LoadFailed,
                error: Some("The source package could not be loaded.".to_owned()),
            },
        ];
        let library_counts = HashMap::from([
            ("missing.source".to_owned(), 18),
            ("update.source".to_owned(), 2),
        ]);
        let candidates = vec![candidate("update.source", "Update Source", json!("2.0.0"))];
        let health = vec![SourceHealthSummary {
            source_id: "update.source".to_owned(),
            runtime: SourceRuntimeHealth::Healthy,
            sample_count: 4,
            latest_at: Some(Utc::now()),
            latest_operation: Some(SourceOperationClass::Search),
            latest_category: Some(SourceErrorCategory::Network),
            latest_message: Some("The source could not reach its provider.".to_owned()),
        }];

        let statuses =
            build_source_statuses(installed, packages, library_counts, candidates, health);

        assert_eq!(statuses.len(), 3);
        assert_eq!(statuses[0].source_id, "missing.source");
        assert_eq!(statuses[0].presence, "missing");
        assert_eq!(statuses[0].library_manga_count, 18);
        assert_eq!(statuses[1].source_id, "broken.source");
        assert_eq!(statuses[1].load, "load_failed");
        assert_eq!(statuses[2].source_id, "update.source");
        assert_eq!(statuses[2].freshness, "update_available");
        assert_eq!(statuses[2].installed_version, Some(json!("1.0.0")));
        assert_eq!(statuses[2].available_version, Some(json!("2.0.0")));
        assert_eq!(
            statuses[2].selected_list_id.as_deref(),
            Some("list-selected")
        );
        assert_eq!(statuses[2].runtime, "healthy");
        assert_eq!(statuses[2].latest_operation, Some("search"));
    }

    #[test]
    fn source_status_ignores_same_id_candidate_from_wrong_package_format() {
        let package = SourcePackageStatus {
            package_label: "fixture.aix".to_owned(),
            kind: SourcePackageKind::Aidoku,
            source_ids: vec![SourceId::new("fixture".to_owned())],
            load: SourcePackageLoadState::LoadFailed,
            error: Some("The source package could not be loaded.".to_owned()),
        };

        let statuses = build_source_statuses(
            Vec::new(),
            vec![package],
            HashMap::new(),
            vec![candidate("fixture", "Fixture", json!("2.0.0"))],
            Vec::new(),
        );

        assert_eq!(statuses[0].catalog, "unavailable");
        assert!(statuses[0].selected_list_id.is_none());
    }

    #[test]
    fn missing_source_does_not_guess_between_package_formats() {
        let mut aidoku_candidate = candidate("fixture", "Fixture", json!(2));
        aidoku_candidate.source_type = SourceListType::Aidoku;

        let statuses = build_source_statuses(
            Vec::new(),
            Vec::new(),
            HashMap::from([("fixture".to_owned(), 1)]),
            vec![
                aidoku_candidate,
                candidate("fixture", "Fixture", json!("2.0.0")),
            ],
            Vec::new(),
        );

        assert_eq!(statuses[0].presence, "missing");
        assert!(statuses[0].selected_list_id.is_none());
        assert_eq!(statuses[0].catalog, "available");
    }

    #[test]
    fn catalog_change_preview_reports_installed_and_missing_sources_losing_coverage() {
        let mut installed_candidate = candidate("installed", "Installed", json!("2.0.0"));
        installed_candidate.list_id = "target".to_owned();
        let mut missing_candidate = candidate("missing", "Missing", json!("2.0.0"));
        missing_candidate.list_id = "target".to_owned();
        let package = SourcePackageStatus {
            package_label: "installed.lnreader.js".to_owned(),
            kind: SourcePackageKind::LnReader,
            source_ids: vec![SourceId::new("installed".to_owned())],
            load: SourcePackageLoadState::Loaded,
            error: None,
        };

        let preview = build_catalog_change_preview(
            "target".to_owned(),
            BTreeSet::from(["target".to_owned()]),
            vec![installed_candidate, missing_candidate],
            vec![source("installed", "Installed", json!("1.0.0"))],
            vec![package],
            HashMap::from([("installed".to_owned(), 2), ("missing".to_owned(), 4)]),
        );

        assert_eq!(preview.affected_sources.len(), 2);
        assert_eq!(preview.affected_sources[0].presence, "installed");
        assert_eq!(preview.affected_sources[1].presence, "missing");
        assert_eq!(preview.affected_sources[1].library_manga_count, 4);
    }

    #[test]
    fn catalog_change_preview_ignores_sources_covered_by_another_active_list() {
        let mut target = candidate("fixture", "Fixture", json!("2.0.0"));
        target.list_id = "target".to_owned();
        let mut alternative = candidate("fixture", "Fixture", json!("1.0.0"));
        alternative.list_id = "alternative".to_owned();
        let package = SourcePackageStatus {
            package_label: "fixture.lnreader.js".to_owned(),
            kind: SourcePackageKind::LnReader,
            source_ids: vec![SourceId::new("fixture".to_owned())],
            load: SourcePackageLoadState::Loaded,
            error: None,
        };

        let preview = build_catalog_change_preview(
            "target".to_owned(),
            BTreeSet::from(["target".to_owned(), "alternative".to_owned()]),
            vec![target, alternative],
            vec![source("fixture", "Fixture", json!("1.0.0"))],
            vec![package],
            HashMap::new(),
        );

        assert!(preview.affected_sources.is_empty());
    }
}
