use std::time::{Duration, Instant};

use axum::extract::{Path, State as StateExtractor};
use axum::routing::post;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::model::SourceId;
use shared::settings::SourceList;
use shared::source::SourceManifest;
use shared::source_catalog::{source_list_id, CatalogStore};
use shared::source_diagnosis::{probe_base_url, BaseUrlProbe};
use shared::source_health::SourceOperationError;
use shared::source_manager::{SourcePackageLoadState, SourcePackageStatus};
use tokio_util::sync::CancellationToken;

use crate::state::State;
use crate::AppError;

const CATALOG_TIMEOUT: Duration = Duration::from_secs(15);
const BASE_URL_TIMEOUT: Duration = Duration::from_secs(10);
const MANGA_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_MANGA_TESTS: usize = 3;

/// Returns the manual source-diagnosis route.
pub fn routes() -> Router<State> {
    Router::new().route("/sources/{source_id}/diagnoses", post(diagnose_source))
}

#[derive(Deserialize)]
struct DiagnosisRequest {
    #[serde(default = "default_refresh_catalog")]
    refresh_catalog: bool,
}

fn default_refresh_catalog() -> bool {
    true
}

#[derive(Debug, Serialize)]
struct DiagnosisStep {
    name: &'static str,
    outcome: &'static str,
    message: String,
    duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tested_item: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    identifier_match: Option<bool>,
}

impl DiagnosisStep {
    fn new(
        name: &'static str,
        outcome: &'static str,
        message: impl Into<String>,
        started: Instant,
    ) -> Self {
        Self {
            name,
            outcome,
            message: message.into(),
            duration_ms: elapsed_millis(started),
            http_status: None,
            package_label: None,
            package_kind: None,
            tested_item: None,
            identifier_match: None,
        }
    }
}

#[derive(Serialize)]
struct SourceDiagnosis {
    source_id: String,
    source_name: String,
    started_at: DateTime<Utc>,
    completed_at: DateTime<Utc>,
    tested_manga_count: usize,
    probable_identifier_change: bool,
    steps: Vec<DiagnosisStep>,
}

async fn diagnose_source(
    StateExtractor(State {
        source_manager,
        database,
        settings,
        catalog_cache_path,
        ..
    }): StateExtractor<State>,
    Path(source_id): Path<String>,
    Json(request): Json<DiagnosisRequest>,
) -> Result<Json<SourceDiagnosis>, AppError> {
    let started_at = Utc::now();
    let source_id = SourceId::new(source_id);
    let (source, manifest, provenance, package, package_file_valid) = {
        let manager = source_manager.lock().await;
        let source = manager
            .sources_by_id
            .get(&source_id)
            .cloned()
            .ok_or(AppError::SourceNotFound)?;
        let manifest = source.manifest();
        let provenance = manager.source_provenance(&source_id);
        let package = manager
            .source_packages
            .iter()
            .find(|package| package.source_ids.contains(&source_id))
            .cloned();
        let package_file_valid = manager
            .file_sources
            .get(source_id.value())
            .and_then(|path| std::fs::metadata(path).ok())
            .is_some_and(|metadata| metadata.is_file() && metadata.len() > 0);
        (source, manifest, provenance, package, package_file_valid)
    };

    let mut steps = Vec::new();
    steps.push(package_step(package, package_file_valid));

    let source_lists = settings.lock().await.source_lists.clone();
    steps.push(
        catalog_step(
            request.refresh_catalog,
            provenance.and_then(|value| value.list_id),
            &source_lists,
            catalog_cache_path,
        )
        .await,
    );

    steps.push(base_url_step(declared_base_url(&manifest)).await);

    let manga_ids = match database
        .get_library_manga_ids_for_source(&source_id, MAX_MANGA_TESTS)
        .await
    {
        Ok(ids) => ids,
        Err(_) => {
            let started = Instant::now();
            steps.push(DiagnosisStep::new(
                "stored_manga_lookup",
                "failed",
                "Stored manga could not be selected for diagnosis.",
                started,
            ));
            Vec::new()
        }
    };
    let tested_manga_count = manga_ids.len();
    let mut probable_identifier_change = false;
    for (index, manga_id) in manga_ids.into_iter().enumerate() {
        let step = manga_step(&source, &manga_id, index + 1).await;
        probable_identifier_change |= step.identifier_match == Some(false);
        steps.push(step);
    }
    if tested_manga_count == 0 && !steps.iter().any(|step| step.name == "stored_manga_lookup") {
        let started = Instant::now();
        steps.push(DiagnosisStep::new(
            "stored_manga_lookup",
            "skipped",
            "No library manga are stored for this source.",
            started,
        ));
    }

    Ok(Json(SourceDiagnosis {
        source_id: source_id.value().clone(),
        source_name: manifest.info.name,
        started_at,
        completed_at: Utc::now(),
        tested_manga_count,
        probable_identifier_change,
        steps,
    }))
}

fn package_step(package: Option<SourcePackageStatus>, package_file_valid: bool) -> DiagnosisStep {
    let started = Instant::now();
    match package {
        Some(package) if package.load == SourcePackageLoadState::Loaded && package_file_valid => {
            let mut step = DiagnosisStep::new(
                "installed_package",
                "passed",
                "The installed package file is present and loaded successfully.",
                started,
            );
            step.package_label = Some(package.package_label);
            step.package_kind = Some(package.kind.as_str());
            step
        }
        Some(package) => {
            let mut step = DiagnosisStep::new(
                "installed_package",
                "failed",
                "The installed package file is missing, empty, or did not load successfully.",
                started,
            );
            step.package_label = Some(package.package_label);
            step.package_kind = Some(package.kind.as_str());
            step
        }
        None => DiagnosisStep::new(
            "installed_package",
            "failed",
            "No installed package record was found for this loaded source.",
            started,
        ),
    }
}

async fn catalog_step(
    refresh: bool,
    installed_list_id: Option<String>,
    source_lists: &[SourceList],
    catalog_cache_path: std::path::PathBuf,
) -> DiagnosisStep {
    let started = Instant::now();
    if !refresh {
        return DiagnosisStep::new(
            "source_list",
            "skipped",
            "Source-list refresh was not requested.",
            started,
        );
    }
    let Some(installed_list_id) = installed_list_id else {
        return DiagnosisStep::new(
            "source_list",
            "skipped",
            "The installed package has no recorded source-list identity.",
            started,
        );
    };
    let Some((order, source_list)) = source_lists.iter().enumerate().find(|(_, source_list)| {
        source_list.enabled && source_list_id(source_list) == installed_list_id
    }) else {
        return DiagnosisStep::new(
            "source_list",
            "skipped",
            "The package's source list is no longer active.",
            started,
        );
    };

    let store = CatalogStore::new(catalog_cache_path);
    match tokio::time::timeout(CATALOG_TIMEOUT, store.refresh_one(source_list, order)).await {
        Ok(Ok(summary)) if summary.last_fetch_error.is_none() => DiagnosisStep::new(
            "source_list",
            "passed",
            format!(
                "The source list refreshed and contains {} sources.",
                summary.candidate_count
            ),
            started,
        ),
        Ok(Ok(_)) => DiagnosisStep::new(
            "source_list",
            "failed",
            "The refresh failed; the last valid cached source list was kept.",
            started,
        ),
        Ok(Err(_)) => DiagnosisStep::new(
            "source_list",
            "failed",
            "The source list could not be refreshed or validated.",
            started,
        ),
        Err(_) => DiagnosisStep::new(
            "source_list",
            "timed_out",
            "The source-list check timed out.",
            started,
        ),
    }
}

async fn base_url_step(base_url: Option<String>) -> DiagnosisStep {
    let started = Instant::now();
    let Some(base_url) = base_url else {
        return DiagnosisStep::new(
            "base_url",
            "skipped",
            "The source does not declare a base URL.",
            started,
        );
    };
    let evidence = probe_base_url(&base_url, BASE_URL_TIMEOUT).await;
    let (outcome, message, http_status) = match evidence {
        BaseUrlProbe::Response(status) if (200..400).contains(&status) => (
            "passed",
            "The source's base URL responded without a redirect.",
            Some(status),
        ),
        BaseUrlProbe::Response(status) => (
            "failed",
            "The source's base URL returned an error response.",
            Some(status),
        ),
        BaseUrlProbe::Redirect(status) => (
            "redirected",
            "The source's base URL redirected. The redirect was recorded but not followed.",
            Some(status),
        ),
        BaseUrlProbe::Unsupported => (
            "skipped",
            "The source does not declare a usable HTTP or HTTPS base URL.",
            None,
        ),
        BaseUrlProbe::TimedOut => ("timed_out", "The base-URL check timed out.", None),
        BaseUrlProbe::Failed => ("failed", "The base URL could not be reached.", None),
    };
    let mut step = DiagnosisStep::new("base_url", outcome, message, started);
    step.http_status = http_status;
    step
}

async fn manga_step(
    source: &shared::source::Source,
    manga_id: &shared::model::MangaId,
    index: usize,
) -> DiagnosisStep {
    let started = Instant::now();
    let token = CancellationToken::new();
    let request = source.get_manga_details(token.clone(), manga_id.value().clone());
    let mut step = match tokio::time::timeout(MANGA_TIMEOUT, request).await {
        Ok(Ok(manga)) => {
            let identifier_match = manga.id == manga_id.value().as_str();
            let mut step = DiagnosisStep::new(
                "stored_manga",
                if identifier_match {
                    "passed"
                } else {
                    "changed"
                },
                if identifier_match {
                    "The stored manga identifier still resolves."
                } else {
                    "The source resolved this manga under a different identifier. No data was changed."
                },
                started,
            );
            step.identifier_match = Some(identifier_match);
            step
        }
        Ok(Err(error)) => DiagnosisStep::new(
            "stored_manga",
            "failed",
            SourceOperationError::classify(error).safe_message(),
            started,
        ),
        Err(_) => {
            token.cancel();
            DiagnosisStep::new(
                "stored_manga",
                "timed_out",
                "The stored manga check timed out.",
                started,
            )
        }
    };
    step.tested_item = Some(index);
    step
}

fn declared_base_url(manifest: &SourceManifest) -> Option<String> {
    manifest
        .info
        .url
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .or_else(|| {
            manifest
                .info
                .urls
                .as_ref()
                .and_then(|urls| urls.iter().find(|value| !value.trim().is_empty()).cloned())
        })
}

fn elapsed_millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn manifest(url: Option<&str>, urls: Option<Vec<&str>>) -> SourceManifest {
        SourceManifest {
            info: shared::source::SourceInfo {
                id: "fixture".to_owned(),
                lang: None,
                languages: None,
                name: "Fixture".to_owned(),
                version: json!(1),
                url: url.map(str::to_owned),
                urls: urls.map(|values| values.into_iter().map(str::to_owned).collect()),
                min_app_version: None,
            },
            config: None,
            source_of_source: None,
        }
    }

    #[test]
    fn declared_base_url_prefers_primary_then_first_non_empty_alternative() {
        assert_eq!(
            declared_base_url(&manifest(
                Some("https://primary.example"),
                Some(vec!["https://other.example"]),
            )),
            Some("https://primary.example".to_owned())
        );
        assert_eq!(
            declared_base_url(&manifest(None, Some(vec!["", "https://other.example"]))),
            Some("https://other.example".to_owned())
        );
    }

    #[test]
    fn diagnosis_step_never_contains_raw_url_or_manga_identifier_fields() {
        let serialized = serde_json::to_value(DiagnosisStep::new(
            "base_url",
            "failed",
            "The base URL could not be reached.",
            Instant::now(),
        ))
        .unwrap();

        assert!(serialized.get("url").is_none());
        assert!(serialized.get("manga_id").is_none());
    }

    #[test]
    fn package_check_requires_a_non_empty_file_and_loaded_inventory() {
        let package = SourcePackageStatus {
            package_label: "fixture.aix".to_owned(),
            kind: shared::source_manager::SourcePackageKind::Aidoku,
            source_ids: vec![SourceId::new("fixture".to_owned())],
            load: SourcePackageLoadState::Loaded,
            error: None,
        };

        assert_eq!(package_step(Some(package.clone()), true).outcome, "passed");
        assert_eq!(package_step(Some(package), false).outcome, "failed");
    }
}
