use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error as StdError,
    fmt, fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use tokio::sync::Mutex;

const SCHEMA_VERSION: u32 = 1;
const MAX_OBSERVATIONS_PER_SOURCE: usize = 20;
const RETENTION_DAYS: i64 = 14;
const FAILURE_WINDOW_HOURS: i64 = 24;
const ITEM_HASH_BYTES: usize = 16;
const MAX_REPORTED_ITEM_IDS: usize = 10;

/// Stable user-facing categories for failures produced by a source operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceErrorCategory {
    Timeout,
    Network,
    Http,
    Parse,
    SourceTrap,
    Incompatible,
    MissingSource,
    Internal,
}

impl SourceErrorCategory {
    pub fn code(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Network => "network",
            Self::Http => "http",
            Self::Parse => "parse",
            Self::SourceTrap => "source_trap",
            Self::Incompatible => "incompatible",
            Self::MissingSource => "missing_source",
            Self::Internal => "internal",
        }
    }

    /// Returns a safe message that contains no remote body, credential, path,
    /// backtrace, or runtime implementation detail.
    pub fn message(self) -> &'static str {
        match self {
            Self::Timeout => "The source timed out.",
            Self::Network => "The source could not be reached.",
            Self::Http => "The source returned an HTTP error.",
            Self::Parse => "The source returned data that could not be read.",
            Self::SourceTrap => "The source stopped while processing the request.",
            Self::Incompatible => "The source is not compatible with this version.",
            Self::MissingSource => "The source is not installed or did not load.",
            Self::Internal => "The source operation could not be completed.",
        }
    }

    pub fn retryable(self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::Network | Self::Http | Self::SourceTrap
        )
    }
}

/// The bounded classes of source work retained in local health history.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceOperationClass {
    Search,
    RefreshChapters,
    RefreshDetails,
}

/// One source-level result for a bounded batch operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceOperationSummary {
    pub source_id: String,
    pub source_name: String,
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub category: Option<SourceErrorCategory>,
    pub message: Option<String>,
    pub failed_item_ids: Vec<String>,
}

impl SourceOperationSummary {
    pub fn new(source_id: impl Into<String>, source_name: impl Into<String>) -> Self {
        Self {
            source_id: source_id.into(),
            source_name: source_name.into(),
            attempted: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            category: None,
            message: None,
            failed_item_ids: Vec::new(),
        }
    }

    pub fn record_success(&mut self) {
        self.attempted += 1;
        self.succeeded += 1;
    }

    pub fn record_failure(&mut self, item_id: &str, error: &SourceOperationError) {
        self.attempted += 1;
        self.failed += 1;
        if self.category.is_none() {
            self.category = Some(error.category());
            self.message = Some(error.safe_message().to_owned());
        }
        if self.failed_item_ids.len() < MAX_REPORTED_ITEM_IDS
            && !self.failed_item_ids.iter().any(|stored| stored == item_id)
        {
            self.failed_item_ids.push(item_id.to_owned());
        }
    }

    pub fn record_skip(&mut self, category: SourceErrorCategory) {
        self.skipped += 1;
        if self.category.is_none() {
            self.category = Some(category);
            self.message = Some(category.message().to_owned());
        }
    }

    pub fn has_problems(&self) -> bool {
        self.failed > 0 || self.skipped > 0
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceOperationReport {
    pub summaries: Vec<SourceOperationSummary>,
}

/// A classified source failure with a safe public message and a private cause.
#[derive(Debug)]
pub struct SourceOperationError {
    category: SourceErrorCategory,
    cause: anyhow::Error,
}

impl SourceOperationError {
    pub fn new(category: SourceErrorCategory, cause: impl Into<anyhow::Error>) -> Self {
        Self {
            category,
            cause: cause.into(),
        }
    }

    pub fn classify(cause: anyhow::Error) -> Self {
        let category = classify_error(&cause);
        Self { category, cause }
    }

    pub fn timeout() -> Self {
        Self::new(
            SourceErrorCategory::Timeout,
            anyhow::anyhow!("source operation timed out"),
        )
    }

    pub fn category(&self) -> SourceErrorCategory {
        self.category
    }

    pub fn safe_message(&self) -> &'static str {
        self.category.message()
    }

    pub fn cause(&self) -> &anyhow::Error {
        &self.cause
    }
}

impl fmt::Display for SourceOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.safe_message())
    }
}

impl StdError for SourceOperationError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.cause.as_ref())
    }
}

impl From<anyhow::Error> for SourceOperationError {
    fn from(value: anyhow::Error) -> Self {
        Self::classify(value)
    }
}

/// One result retained for health calculation. Item keys are stored only as
/// hashes so manga ids and search terms do not enter the diagnostics file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceHealthObservation {
    pub source_id: String,
    pub operation: SourceOperationClass,
    pub category: Option<SourceErrorCategory>,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub item_key_hash: String,
    pub succeeded: bool,
}

impl SourceHealthObservation {
    pub fn success(
        source_id: impl Into<String>,
        operation: SourceOperationClass,
        item_key: &str,
    ) -> Self {
        Self::success_at(source_id, operation, item_key, Utc::now())
    }

    pub fn failure(
        source_id: impl Into<String>,
        operation: SourceOperationClass,
        item_key: &str,
        error: &SourceOperationError,
    ) -> Self {
        Self::failure_at(source_id, operation, item_key, error.category(), Utc::now())
    }

    pub fn failure_with_category(
        source_id: impl Into<String>,
        operation: SourceOperationClass,
        item_key: &str,
        category: SourceErrorCategory,
    ) -> Self {
        Self::failure_at(source_id, operation, item_key, category, Utc::now())
    }

    fn success_at(
        source_id: impl Into<String>,
        operation: SourceOperationClass,
        item_key: &str,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            operation,
            category: None,
            message: None,
            timestamp,
            item_key_hash: hash_item_key(item_key),
            succeeded: true,
        }
    }

    fn failure_at(
        source_id: impl Into<String>,
        operation: SourceOperationClass,
        item_key: &str,
        category: SourceErrorCategory,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            operation,
            category: Some(category),
            message: Some(category.message().to_owned()),
            timestamp,
            item_key_hash: hash_item_key(item_key),
            succeeded: false,
        }
    }
}

/// In-memory operation batch that keeps only the newest persistable samples
/// per source, even when a library contains many thousands of manga.
#[derive(Default)]
pub struct SourceHealthBatch {
    observations: BTreeMap<String, Vec<SourceHealthObservation>>,
}

impl SourceHealthBatch {
    pub fn push(&mut self, observation: SourceHealthObservation) {
        let observations = self
            .observations
            .entry(observation.source_id.clone())
            .or_default();
        observations.push(observation);
        if observations.len() > MAX_OBSERVATIONS_PER_SOURCE {
            observations.remove(0);
        }
    }

    pub fn into_observations(self) -> Vec<SourceHealthObservation> {
        self.observations.into_values().flatten().collect()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRuntimeHealth {
    Healthy,
    Failing,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceHealthSummary {
    pub source_id: String,
    pub runtime: SourceRuntimeHealth,
    pub sample_count: usize,
    pub latest_at: Option<DateTime<Utc>>,
    pub latest_category: Option<SourceErrorCategory>,
    pub latest_message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PersistedHealth {
    schema_version: u32,
    observations: BTreeMap<String, Vec<SourceHealthObservation>>,
}

impl Default for PersistedHealth {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            observations: BTreeMap::new(),
        }
    }
}

/// Thread-safe, file-backed source health history with fixed time and count
/// bounds. Clones share one in-memory record and one atomic persistence path.
#[derive(Clone)]
pub struct SourceHealthStore {
    path: PathBuf,
    inner: Arc<Mutex<PersistedHealth>>,
}

impl SourceHealthStore {
    pub fn open(path: PathBuf) -> Self {
        let mut persisted = match fs::read(&path) {
            Ok(contents) => match serde_json::from_slice::<PersistedHealth>(&contents) {
                Ok(value) if value.schema_version == SCHEMA_VERSION => value,
                Ok(value) => {
                    log::warn!(
                        "ignoring source health schema version {}",
                        value.schema_version
                    );
                    PersistedHealth::default()
                }
                Err(error) => {
                    log::warn!("ignoring invalid source health history: {error}");
                    PersistedHealth::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                PersistedHealth::default()
            }
            Err(error) => {
                log::warn!("couldn't read source health history: {error}");
                PersistedHealth::default()
            }
        };
        prune(&mut persisted, Utc::now());
        Self {
            path,
            inner: Arc::new(Mutex::new(persisted)),
        }
    }

    /// Adds a complete operation batch and publishes the bounded result once.
    pub async fn record_batch(&self, observations: Vec<SourceHealthObservation>) -> Result<()> {
        if observations.is_empty() {
            return Ok(());
        }
        let now = Utc::now();
        let mut persisted = self.inner.lock().await;
        for observation in observations {
            persisted
                .observations
                .entry(observation.source_id.clone())
                .or_default()
                .push(observation);
        }
        prune(&mut persisted, now);
        atomic_write(&self.path, &persisted)
    }

    pub async fn summaries(&self) -> Vec<SourceHealthSummary> {
        let persisted = self.inner.lock().await;
        persisted
            .observations
            .iter()
            .map(|(source_id, observations)| summarize(source_id, observations, Utc::now()))
            .collect()
    }
}

fn prune(persisted: &mut PersistedHealth, now: DateTime<Utc>) {
    let cutoff = now - Duration::days(RETENTION_DAYS);
    persisted.observations.retain(|_, observations| {
        observations.retain(|observation| observation.timestamp >= cutoff);
        observations.sort_by_key(|observation| observation.timestamp);
        if observations.len() > MAX_OBSERVATIONS_PER_SOURCE {
            observations.drain(..observations.len() - MAX_OBSERVATIONS_PER_SOURCE);
        }
        !observations.is_empty()
    });
}

fn summarize(
    source_id: &str,
    observations: &[SourceHealthObservation],
    now: DateTime<Utc>,
) -> SourceHealthSummary {
    let latest = observations.last();
    let latest_success = observations
        .iter()
        .rev()
        .find(|observation| observation.succeeded)
        .map(|observation| observation.timestamp);
    let cutoff = now - Duration::hours(FAILURE_WINDOW_HOURS);
    let active_failures = observations
        .iter()
        .filter(|observation| {
            !observation.succeeded
                && observation.timestamp >= cutoff
                && latest_success.is_none_or(|success| observation.timestamp > success)
        })
        .collect::<Vec<_>>();
    let distinct_items = active_failures
        .iter()
        .map(|observation| &observation.item_key_hash)
        .collect::<BTreeSet<_>>()
        .len();
    let distinct_operations = active_failures
        .iter()
        .map(|observation| observation.operation)
        .collect::<BTreeSet<_>>()
        .len();
    let runtime = if latest.is_some_and(|observation| observation.succeeded) {
        SourceRuntimeHealth::Healthy
    } else if active_failures.len() >= 3 && (distinct_items >= 2 || distinct_operations >= 2) {
        SourceRuntimeHealth::Failing
    } else {
        SourceRuntimeHealth::Unknown
    };

    SourceHealthSummary {
        source_id: source_id.to_owned(),
        runtime,
        sample_count: observations.len(),
        latest_at: latest.map(|observation| observation.timestamp),
        latest_category: latest.and_then(|observation| observation.category),
        latest_message: latest.and_then(|observation| observation.message.clone()),
    }
}

fn hash_item_key(item_key: &str) -> String {
    let digest = Sha256::digest(item_key.as_bytes());
    hex::encode(&digest[..ITEM_HASH_BYTES])
}

fn classify_error(error: &anyhow::Error) -> SourceErrorCategory {
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<reqwest::Error>() {
            if error.is_timeout() {
                return SourceErrorCategory::Timeout;
            }
            if error.status().is_some() {
                return SourceErrorCategory::Http;
            }
            if error.is_connect() || error.is_request() {
                return SourceErrorCategory::Network;
            }
            if error.is_decode() {
                return SourceErrorCategory::Parse;
            }
        }
        if cause.downcast_ref::<serde_json::Error>().is_some()
            || cause.downcast_ref::<url::ParseError>().is_some()
        {
            return SourceErrorCategory::Parse;
        }
        if let Some(error) = cause.downcast_ref::<std::io::Error>() {
            return match error.kind() {
                std::io::ErrorKind::TimedOut => SourceErrorCategory::Timeout,
                std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::HostUnreachable
                | std::io::ErrorKind::NetworkDown
                | std::io::ErrorKind::NetworkUnreachable
                | std::io::ErrorKind::NotConnected => SourceErrorCategory::Network,
                _ => SourceErrorCategory::Internal,
            };
        }
    }

    let description = format!("{error:#}").to_ascii_lowercase();
    if description.contains("timed out") || description.contains("timeout") {
        SourceErrorCategory::Timeout
    } else if description.contains("http status") || description.contains("status code") {
        SourceErrorCategory::Http
    } else if description.contains("parse")
        || description.contains("invalid json")
        || description.contains("decode")
    {
        SourceErrorCategory::Parse
    } else if description.contains("incompatible") || description.contains("unsupported") {
        SourceErrorCategory::Incompatible
    } else if description.contains("trap")
        || description.contains("source error")
        || description.contains("javascript")
        || description.contains("wasm")
        || description.contains("dex")
    {
        SourceErrorCategory::SourceTrap
    } else if description.contains("connect")
        || description.contains("network")
        || description.contains("dns")
        || description.contains("tls")
    {
        SourceErrorCategory::Network
    } else {
        SourceErrorCategory::Internal
    }
}

fn atomic_write(path: &Path, persisted: &PersistedHealth) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).context("couldn't create source health directory")?;
    let mut temporary =
        NamedTempFile::new_in(parent).context("couldn't create temporary source health file")?;
    serde_json::to_writer(temporary.as_file_mut(), persisted)
        .context("couldn't serialize source health history")?;
    temporary
        .as_file_mut()
        .write_all(b"\n")
        .context("couldn't finish source health write")?;
    temporary
        .as_file_mut()
        .flush()
        .context("couldn't flush source health history")?;
    temporary
        .as_file()
        .sync_all()
        .context("couldn't sync source health history")?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .context("couldn't publish source health history")?;
    sync_parent_directory(parent);
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) {
    if let Err(error) = fs::File::open(parent).and_then(|directory| directory.sync_all()) {
        log::warn!("couldn't sync source health directory: {error}");
    }
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) {}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn failure(
        source: &str,
        operation: SourceOperationClass,
        item: &str,
        timestamp: DateTime<Utc>,
    ) -> SourceHealthObservation {
        SourceHealthObservation::failure_at(
            source,
            operation,
            item,
            SourceErrorCategory::Timeout,
            timestamp,
        )
    }

    #[test]
    fn repeated_failures_need_distinct_evidence_before_marking_failing() {
        let now = Utc::now();
        let same_item = vec![
            failure("source", SourceOperationClass::Search, "one", now),
            failure("source", SourceOperationClass::Search, "one", now),
            failure("source", SourceOperationClass::Search, "one", now),
        ];
        assert_eq!(
            summarize("source", &same_item, now).runtime,
            SourceRuntimeHealth::Unknown
        );

        let distinct_items = vec![
            failure("source", SourceOperationClass::Search, "one", now),
            failure("source", SourceOperationClass::Search, "two", now),
            failure("source", SourceOperationClass::Search, "two", now),
        ];
        assert_eq!(
            summarize("source", &distinct_items, now).runtime,
            SourceRuntimeHealth::Failing
        );
    }

    #[test]
    fn later_success_clears_active_failure_state() {
        let now = Utc::now();
        let mut observations = vec![
            failure("source", SourceOperationClass::Search, "one", now),
            failure("source", SourceOperationClass::Search, "two", now),
            failure("source", SourceOperationClass::Search, "two", now),
        ];
        observations.push(SourceHealthObservation::success_at(
            "source",
            SourceOperationClass::Search,
            "three",
            now + Duration::seconds(1),
        ));

        assert_eq!(
            summarize("source", &observations, now + Duration::seconds(1)).runtime,
            SourceRuntimeHealth::Healthy
        );
    }

    #[test]
    fn operation_summary_groups_equivalent_failures_and_bounds_item_ids() {
        let error = SourceOperationError::timeout();
        let mut summary = SourceOperationSummary::new("source", "Source");
        summary.record_success();
        for index in 0..15 {
            summary.record_failure(&format!("item-{index}"), &error);
        }
        summary.record_skip(SourceErrorCategory::MissingSource);

        assert_eq!(summary.attempted, 16);
        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.failed, 15);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.category, Some(SourceErrorCategory::Timeout));
        assert_eq!(summary.message.as_deref(), Some("The source timed out."));
        assert_eq!(summary.failed_item_ids.len(), MAX_REPORTED_ITEM_IDS);
    }

    #[test]
    fn in_memory_batch_is_bounded_per_source() {
        let now = Utc::now();
        let mut batch = SourceHealthBatch::default();
        for index in 0..25 {
            batch.push(failure(
                "source",
                SourceOperationClass::RefreshDetails,
                &format!("item-{index}"),
                now + Duration::seconds(index),
            ));
        }

        let observations = batch.into_observations();
        assert_eq!(observations.len(), MAX_OBSERVATIONS_PER_SOURCE);
        assert_eq!(
            observations
                .first()
                .map(|observation| observation.timestamp),
            Some(now + Duration::seconds(5))
        );
    }

    #[tokio::test]
    async fn persisted_history_is_bounded_and_uses_hashed_item_keys() {
        let directory = tempdir().expect("create health directory");
        let path = directory.path().join("source_health.json");
        let store = SourceHealthStore::open(path.clone());
        let now = Utc::now();
        let observations = (0..25)
            .map(|index| {
                failure(
                    "source",
                    SourceOperationClass::RefreshChapters,
                    &format!("private-manga-{index}"),
                    now + Duration::seconds(index),
                )
            })
            .collect();

        store
            .record_batch(observations)
            .await
            .expect("persist observations");

        let persisted: PersistedHealth =
            serde_json::from_slice(&fs::read(path).expect("read health history"))
                .expect("parse health history");
        let observations = &persisted.observations["source"];
        assert_eq!(observations.len(), MAX_OBSERVATIONS_PER_SOURCE);
        assert!(observations
            .iter()
            .all(|observation| !observation.item_key_hash.contains("private-manga")));
    }

    #[test]
    fn classification_and_public_messages_do_not_expose_causes() {
        let error = SourceOperationError::classify(anyhow::anyhow!(
            "WASM trap at /secret/path with cookie=private"
        ));

        assert_eq!(error.category(), SourceErrorCategory::SourceTrap);
        assert_eq!(error.to_string(), SourceErrorCategory::SourceTrap.message());
        assert!(!error.to_string().contains("secret"));
        assert!(!error.to_string().contains("cookie"));
    }
}
