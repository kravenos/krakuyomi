use std::{
    cmp::Ordering,
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use url::Url;

use crate::{
    model::{SourceId, SourceInformation},
    settings::{SourceList, SourceListType},
    usecases::{fetch_source_list::fetch_source_list, resolve_source_list::resolve_source_list},
};

const CATALOG_SCHEMA_VERSION: u8 = 1;
const MAX_STORED_ERROR_CHARS: usize = 512;

/// A validated source-list entry with the exact catalog that supplied it.
#[derive(Clone, Debug)]
pub struct CatalogCandidate {
    pub list_id: String,
    pub source_type: SourceListType,
    pub provider_url: Url,
    pub resolved_provider_url: Url,
    pub list_order: usize,
    pub fetched_at: DateTime<Utc>,
    pub last_fetch_error: Option<String>,
    pub source: SourceInformation,
    pub raw: Value,
    pub selected: bool,
}

/// The exact provenance persisted beside an installed source package.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SourceProvenance {
    #[serde(rename = "from", default, skip_serializing_if = "Option::is_none")]
    pub source_of_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_url: Option<Url>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_provider_url: Option<Url>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<Value>,
}

impl From<String> for SourceProvenance {
    fn from(source_of_source: String) -> Self {
        Self {
            source_of_source: Some(source_of_source),
            ..Self::default()
        }
    }
}

impl CatalogCandidate {
    pub fn provenance(&self) -> SourceProvenance {
        SourceProvenance {
            source_of_source: self.source.source_of_source.clone(),
            list_id: Some(self.list_id.clone()),
            provider_url: Some(self.provider_url.clone()),
            resolved_provider_url: Some(self.resolved_provider_url.clone()),
            version: Some(self.source.version.clone()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CatalogEntry {
    id: SourceId,
    name: String,
    version: Value,
    languages: Vec<String>,
    raw: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CatalogValidation {
    candidate_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CatalogCache {
    schema_version: u8,
    list_id: String,
    source_type: SourceListType,
    provider_url: Url,
    resolved_provider_url: Url,
    configured_order: usize,
    fetched_at: DateTime<Utc>,
    validation: CatalogValidation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_fetch_error: Option<String>,
    candidates: Vec<CatalogEntry>,
}

/// Persistent last-known-good source catalogs.
#[derive(Clone, Debug)]
pub struct CatalogStore {
    root: PathBuf,
}

impl CatalogStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Refreshes every configured list independently. A failed list keeps its
    /// previous valid candidates, and never discards another list's result.
    pub async fn refresh(&self, source_lists: &[SourceList]) -> Result<Vec<CatalogCandidate>> {
        if source_lists.is_empty() {
            return Ok(Vec::new());
        }

        fs::create_dir_all(&self.root).with_context(|| {
            format!(
                "couldn't create source catalog cache directory {}",
                self.root.display()
            )
        })?;
        let client = crate::tls::client_builder()
            .build()
            .context("failed to create source catalog HTTP client")?;

        let mut caches = Vec::new();
        let mut failures = 0usize;
        for (configured_order, source_list) in source_lists.iter().enumerate() {
            let list_id = source_list_id(source_list);
            let previous = self
                .load_cache(&list_id)
                .ok()
                .filter(|cache| cache_matches_source_list(cache, source_list, &list_id));
            let resolved_provider_url = resolve_source_list(source_list).await;
            let refreshed = fetch_source_list(&client, &resolved_provider_url)
                .await
                .and_then(|value| {
                    build_cache(
                        source_list,
                        configured_order,
                        resolved_provider_url.clone(),
                        value,
                    )
                })
                .and_then(|cache| {
                    self.save_cache(&cache)?;
                    Ok(cache)
                });

            match refreshed {
                Ok(cache) => caches.push(cache),
                Err(error) => {
                    failures += 1;
                    let message = bounded_error(&error);
                    log::warn!(
                        "source catalog refresh failed for list {}: {}",
                        list_id,
                        message
                    );
                    if let Some(mut cache) = previous {
                        cache.configured_order = configured_order;
                        cache.last_fetch_error = Some(message);
                        if let Err(save_error) = self.save_cache(&cache) {
                            log::warn!(
                                "couldn't persist source catalog failure state for {}: {save_error:#}",
                                list_id
                            );
                        }
                        caches.push(cache);
                    }
                }
            }
        }

        if caches.is_empty() && failures > 0 {
            bail!("couldn't refresh any configured source catalog ({failures} failed)");
        }

        Ok(candidates_from_caches(caches))
    }

    /// Loads cached candidates for the currently configured lists without
    /// contacting the network.
    pub fn load(&self, source_lists: &[SourceList]) -> Result<Vec<CatalogCandidate>> {
        let mut caches = Vec::new();
        for (configured_order, source_list) in source_lists.iter().enumerate() {
            let list_id = source_list_id(source_list);
            match self.load_cache(&list_id) {
                Ok(mut cache) if cache_matches_source_list(&cache, source_list, &list_id) => {
                    cache.configured_order = configured_order;
                    caches.push(cache);
                }
                Ok(_) => log::warn!("ignoring mismatched source catalog cache {list_id}"),
                Err(error) if self.cache_path(&list_id).exists() => {
                    log::warn!("ignoring invalid source catalog cache {list_id}: {error:#}")
                }
                Err(_) => {}
            }
        }
        Ok(candidates_from_caches(caches))
    }

    /// Finds the exact cached candidate named by an install request.
    pub fn find(
        &self,
        source_lists: &[SourceList],
        list_id: &str,
        source_id: &SourceId,
        version: &Value,
    ) -> Result<CatalogCandidate> {
        let configured = source_lists
            .iter()
            .any(|source_list| source_list_id(source_list) == list_id);
        if !configured {
            bail!("source catalog is no longer configured");
        }

        self.load(source_lists)?
            .into_iter()
            .find(|candidate| {
                candidate.list_id == list_id
                    && candidate.source.id == *source_id
                    && candidate.source.version == *version
            })
            .context("the selected source version is no longer in the cached catalog")
    }

    fn cache_path(&self, list_id: &str) -> PathBuf {
        self.root.join(format!("{list_id}.json"))
    }

    fn load_cache(&self, list_id: &str) -> Result<CatalogCache> {
        let path = self.cache_path(list_id);
        let cache: CatalogCache = serde_json::from_reader(
            fs::File::open(&path)
                .with_context(|| format!("couldn't open source catalog cache {list_id}"))?,
        )
        .with_context(|| format!("couldn't parse source catalog cache {list_id}"))?;
        if cache.schema_version != CATALOG_SCHEMA_VERSION {
            bail!("unsupported source catalog cache version");
        }
        if cache.validation.candidate_count != cache.candidates.len() {
            bail!("source catalog cache validation count does not match its contents");
        }
        Ok(cache)
    }

    fn save_cache(&self, cache: &CatalogCache) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        let mut temporary = NamedTempFile::new_in(&self.root)
            .context("couldn't create temporary source catalog cache")?;
        serde_json::to_writer(temporary.as_file_mut(), cache)
            .context("couldn't serialize source catalog cache")?;
        temporary
            .as_file_mut()
            .write_all(b"\n")
            .context("couldn't finish source catalog cache write")?;
        temporary
            .as_file_mut()
            .flush()
            .context("couldn't flush source catalog cache")?;
        temporary
            .as_file()
            .sync_all()
            .context("couldn't sync source catalog cache")?;
        temporary
            .persist(self.cache_path(&cache.list_id))
            .map_err(|error| error.error)
            .context("couldn't publish source catalog cache")?;
        sync_parent_directory(&self.root);
        Ok(())
    }
}

/// Returns the exact normalized URL used as the catalog's identity input.
pub fn normalized_source_list_url(source_list: &SourceList) -> Url {
    let mut url = source_list.url.clone();
    url.set_fragment(None);
    let default_port = matches!(
        (url.scheme(), url.port()),
        ("http", Some(80)) | ("https", Some(443))
    );
    if default_port {
        let _ = url.set_port(None);
    }
    url
}

/// Stable collision-resistant identity for one URL and index format.
pub fn source_list_id(source_list: &SourceList) -> String {
    let normalized = normalized_source_list_url(source_list);
    let mut digest = Sha256::new();
    digest.update(format!(
        "{}\n{}",
        source_type_key(source_list.source_type),
        normalized
    ));
    hex::encode(digest.finalize())
}

fn source_type_key(source_type: SourceListType) -> &'static str {
    match source_type {
        SourceListType::Aidoku => "aidoku",
        SourceListType::LnReader => "lnreader",
        SourceListType::Mangayomi => "mangayomi",
        SourceListType::Keiyoushi => "keiyoushi",
    }
}

fn build_cache(
    source_list: &SourceList,
    configured_order: usize,
    resolved_provider_url: Url,
    value: Value,
) -> Result<CatalogCache> {
    let mut raw_entries = match value {
        Value::Array(entries) => entries,
        Value::Object(mut object) => object
            .remove("sources")
            .and_then(|value| value.as_array().cloned())
            .context("source catalog has no source array")?,
        _ => bail!("source catalog is not a JSON array or object"),
    };
    normalize_entry_ids(&mut raw_entries);

    let mut parsed = raw_entries
        .iter()
        .filter_map(
            |raw| match serde_json::from_value::<SourceInformation>(raw.clone()) {
                Ok(source) => Some((source, raw.clone())),
                Err(error) => {
                    log::warn!("ignoring invalid source catalog entry: {error}");
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    if source_list.source_type == SourceListType::Keiyoushi {
        let mut entries = parsed
            .iter()
            .map(|(source, _)| {
                (
                    source.id.value().to_string(),
                    source.languages.first().cloned(),
                )
            })
            .collect::<Vec<_>>();
        crate::usecases::fetch_source_list::expand_keiyoushi_ids(&mut entries);
        for ((source, _), (id, _)) in parsed.iter_mut().zip(entries) {
            source.id = SourceId::new(id);
        }
    }

    let candidates = parsed
        .into_iter()
        .filter(|(source, raw)| candidate_is_compatible(source_list.source_type, source, raw))
        .map(|(source, raw)| CatalogEntry {
            id: source.id,
            name: source.name,
            version: source.version,
            languages: source.languages,
            raw,
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        bail!("source catalog contains no supported installable sources");
    }

    Ok(CatalogCache {
        schema_version: CATALOG_SCHEMA_VERSION,
        list_id: source_list_id(source_list),
        source_type: source_list.source_type,
        provider_url: normalized_source_list_url(source_list),
        resolved_provider_url,
        configured_order,
        fetched_at: Utc::now(),
        validation: CatalogValidation {
            candidate_count: candidates.len(),
        },
        last_fetch_error: None,
        candidates,
    })
}

fn normalize_entry_ids(entries: &mut [Value]) {
    for entry in entries {
        let Value::Object(object) = entry else {
            continue;
        };
        if let Some(Value::Number(id)) = object.get("id") {
            object.insert("id".to_string(), Value::String(id.to_string()));
        } else if let Some(Value::String(package)) = object.get("pkg") {
            object.insert("id".to_string(), Value::String(package.clone()));
        }
    }
}

fn candidate_is_compatible(
    source_type: SourceListType,
    source: &SourceInformation,
    raw: &Value,
) -> bool {
    if source.id.value().is_empty() || source.name.trim().is_empty() {
        return false;
    }
    let version_compatible = match source_type {
        SourceListType::Aidoku => source.version.is_number(),
        SourceListType::LnReader | SourceListType::Mangayomi | SourceListType::Keiyoushi => source
            .version
            .as_str()
            .is_some_and(|version| !version.trim().is_empty()),
    };
    if !version_compatible {
        return false;
    }
    let install_field_present = match source_type {
        SourceListType::Aidoku => raw.get("downloadURL").or_else(|| raw.get("file")).is_some(),
        SourceListType::LnReader => raw.get("url").is_some(),
        SourceListType::Mangayomi => {
            raw.get("sourceCodeUrl").is_some()
                && raw.get("itemType").and_then(Value::as_u64).unwrap_or(0) != 1
        }
        SourceListType::Keiyoushi => raw.get("apk").or_else(|| raw.get("downloadURL")).is_some(),
    };
    install_field_present
}

fn cache_matches_source_list(
    cache: &CatalogCache,
    source_list: &SourceList,
    list_id: &str,
) -> bool {
    cache.list_id == list_id
        && cache.source_type == source_list.source_type
        && cache.provider_url == normalized_source_list_url(source_list)
}

fn candidates_from_caches(caches: Vec<CatalogCache>) -> Vec<CatalogCandidate> {
    let mut candidates = caches
        .into_iter()
        .flat_map(|cache| {
            cache.candidates.into_iter().map(move |entry| {
                let source_of_source =
                    crate::usecases::resolve_source_list::source_list_key(&SourceList {
                        url: cache.provider_url.clone(),
                        source_type: cache.source_type,
                    });
                CatalogCandidate {
                    list_id: cache.list_id.clone(),
                    source_type: cache.source_type,
                    provider_url: cache.provider_url.clone(),
                    resolved_provider_url: cache.resolved_provider_url.clone(),
                    list_order: cache.configured_order,
                    fetched_at: cache.fetched_at.clone(),
                    last_fetch_error: cache.last_fetch_error.clone(),
                    source: SourceInformation {
                        id: entry.id,
                        name: entry.name,
                        version: entry.version,
                        languages: entry.languages,
                        source_of_source: Some(source_of_source),
                        missing: false,
                        catalog_list_id: None,
                        provider_url: None,
                        resolved_provider_url: None,
                        installed_version: None,
                    },
                    raw: entry.raw,
                    selected: false,
                }
            })
        })
        .collect::<Vec<_>>();
    mark_selected_candidates(&mut candidates);
    candidates.sort_by(|left, right| {
        left.source
            .name
            .cmp(&right.source.name)
            .then_with(|| left.source.id.value().cmp(right.source.id.value()))
            .then_with(|| right.selected.cmp(&left.selected))
            .then_with(|| compare_candidate_preference(right, left))
    });
    candidates
}

fn mark_selected_candidates(candidates: &mut [CatalogCandidate]) {
    // Source ids are only comparable inside the same runtime format. An
    // Aidoku package and an LNReader plugin with the same id are independent
    // candidates and must each have a selected provider.
    let mut selected_by_source: HashMap<(SourceId, &'static str), usize> = HashMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        selected_by_source
            .entry((
                candidate.source.id.clone(),
                source_type_key(candidate.source_type),
            ))
            .and_modify(|selected| {
                if compare_candidate_preference(candidate, &candidates[*selected])
                    == Ordering::Greater
                {
                    *selected = index;
                }
            })
            .or_insert(index);
    }
    for index in selected_by_source.into_values() {
        candidates[index].selected = true;
    }
}

fn compare_candidate_preference(left: &CatalogCandidate, right: &CatalogCandidate) -> Ordering {
    compare_versions(
        left.source_type,
        &left.source.version,
        &right.source.version,
    )
    .then_with(|| right.list_order.cmp(&left.list_order))
    .then_with(|| right.provider_url.as_str().cmp(left.provider_url.as_str()))
}

fn compare_versions(source_type: SourceListType, left: &Value, right: &Value) -> Ordering {
    if source_type == SourceListType::Aidoku {
        return left
            .as_f64()
            .unwrap_or(f64::NEG_INFINITY)
            .total_cmp(&right.as_f64().unwrap_or(f64::NEG_INFINITY));
    }
    let left = left.as_str().unwrap_or_default().trim_start_matches('v');
    let right = right.as_str().unwrap_or_default().trim_start_matches('v');
    match (semver::Version::parse(left), semver::Version::parse(right)) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => compare_dotted_versions(left, right).then_with(|| left.cmp(right)),
    }
}

fn compare_dotted_versions(left: &str, right: &str) -> Ordering {
    let parse = |version: &str| {
        version
            .split(['.', '-', '+'])
            .take(4)
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parse(left).cmp(&parse(right))
}

fn bounded_error(error: &anyhow::Error) -> String {
    error
        .to_string()
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_STORED_ERROR_CHARS)
        .collect()
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) {
    if let Err(error) = fs::File::open(parent).and_then(|directory| directory.sync_all()) {
        log::warn!(
            "couldn't sync source catalog directory {}: {error}",
            parent.display()
        );
    }
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn source_list(url: &str, source_type: SourceListType) -> SourceList {
        SourceList {
            url: Url::parse(url).expect("test URL is valid"),
            source_type,
        }
    }

    fn cache(list: &SourceList, order: usize, id: &str, version: Value) -> CatalogCache {
        CatalogCache {
            schema_version: CATALOG_SCHEMA_VERSION,
            list_id: source_list_id(list),
            source_type: list.source_type,
            provider_url: normalized_source_list_url(list),
            resolved_provider_url: list.url.clone(),
            configured_order: order,
            fetched_at: Utc::now(),
            validation: CatalogValidation { candidate_count: 1 },
            last_fetch_error: None,
            candidates: vec![CatalogEntry {
                id: SourceId::new(id.to_string()),
                name: "Fixture".to_string(),
                version,
                languages: vec!["en".to_string()],
                raw: serde_json::json!({"id": id, "name": "Fixture", "url": "https://example.com/source.js"}),
            }],
        }
    }

    #[test]
    fn list_identity_removes_fragments_and_default_ports() {
        let first = source_list(
            "https://EXAMPLE.com:443/catalog.json#section",
            SourceListType::LnReader,
        );
        let second = source_list("https://example.com/catalog.json", SourceListType::LnReader);
        assert_eq!(source_list_id(&first), source_list_id(&second));
        assert_eq!(
            normalized_source_list_url(&first).as_str(),
            "https://example.com/catalog.json"
        );
    }

    #[test]
    fn list_identity_includes_the_index_format() {
        let aidoku = source_list("https://example.com/index.json", SourceListType::Aidoku);
        let lnreader = source_list("https://example.com/index.json", SourceListType::LnReader);
        assert_ne!(source_list_id(&aidoku), source_list_id(&lnreader));
    }

    #[test]
    fn cache_roundtrip_preserves_candidates_and_failure_state() {
        let directory = tempdir().expect("create cache directory");
        let store = CatalogStore::new(directory.path().to_path_buf());
        let list = source_list("https://example.com/index.json", SourceListType::LnReader);
        let mut expected = cache(&list, 0, "fixture", serde_json::json!("1.2.3"));
        expected.last_fetch_error = Some("offline".to_string());

        store.save_cache(&expected).expect("save cache");
        let actual = store.load_cache(&expected.list_id).expect("load cache");

        assert_eq!(actual.candidates.len(), 1);
        assert_eq!(actual.candidates[0].id.value(), "fixture");
        assert_eq!(actual.last_fetch_error.as_deref(), Some("offline"));
    }

    #[test]
    fn catalog_validation_keeps_valid_entries_when_one_entry_is_bad() {
        let list = source_list("https://example.com/index.json", SourceListType::LnReader);
        let cache = build_cache(
            &list,
            0,
            list.url.clone(),
            serde_json::json!([
                {
                    "id": "valid",
                    "name": "Valid",
                    "version": "1.2.3",
                    "url": "https://example.com/valid.js"
                },
                {"id": "bad", "name": "Missing version"}
            ]),
        )
        .expect("one bad entry must not poison the valid catalog");

        assert_eq!(cache.candidates.len(), 1);
        assert_eq!(cache.candidates[0].id.value(), "valid");
    }

    #[test]
    fn deterministic_selection_prefers_version_then_list_order_then_url() {
        let first = source_list("https://z.example/index.json", SourceListType::LnReader);
        let second = source_list("https://a.example/index.json", SourceListType::LnReader);
        let mut candidates = candidates_from_caches(vec![
            cache(&first, 0, "fixture", serde_json::json!("1.0.0")),
            cache(&second, 1, "fixture", serde_json::json!("2.0.0")),
        ]);
        let selected = candidates
            .iter()
            .find(|candidate| candidate.selected)
            .expect("one candidate selected");
        assert_eq!(selected.source.version, serde_json::json!("2.0.0"));

        candidates = candidates_from_caches(vec![
            cache(&first, 0, "fixture", serde_json::json!("2.0.0")),
            cache(&second, 1, "fixture", serde_json::json!("2.0.0")),
        ]);
        let selected = candidates
            .iter()
            .find(|candidate| candidate.selected)
            .expect("one candidate selected");
        assert_eq!(selected.provider_url, first.url);

        candidates = candidates_from_caches(vec![
            cache(&first, 0, "fixture", serde_json::json!("2.0.0")),
            cache(&second, 0, "fixture", serde_json::json!("2.0.0")),
        ]);
        let selected = candidates
            .iter()
            .find(|candidate| candidate.selected)
            .expect("one candidate selected");
        assert_eq!(selected.provider_url, second.url);
    }

    #[test]
    fn different_source_formats_are_selected_independently() {
        let aidoku = source_list("https://example.com/aidoku.json", SourceListType::Aidoku);
        let lnreader = source_list(
            "https://example.com/lnreader.json",
            SourceListType::LnReader,
        );
        let candidates = candidates_from_caches(vec![
            cache(&aidoku, 0, "fixture", serde_json::json!(2)),
            cache(&lnreader, 1, "fixture", serde_json::json!("1.0.0")),
        ]);

        assert_eq!(
            candidates
                .iter()
                .filter(|candidate| candidate.selected)
                .count(),
            2
        );
    }

    #[test]
    fn exact_find_rejects_a_different_version() {
        let directory = tempdir().expect("create cache directory");
        let store = CatalogStore::new(directory.path().to_path_buf());
        let list = source_list("https://example.com/index.json", SourceListType::LnReader);
        let cache = cache(&list, 0, "fixture", serde_json::json!("1.2.3"));
        let list_id = cache.list_id.clone();
        store.save_cache(&cache).expect("save cache");

        assert!(store
            .find(
                &[list],
                &list_id,
                &SourceId::new("fixture".to_string()),
                &serde_json::json!("9.9.9"),
            )
            .is_err());
    }

    #[test]
    fn exact_provenance_is_written_beside_the_package() {
        let directory = tempdir().expect("create source directory");
        let package = directory.path().join("fixture.lnreader.js");
        fs::write(&package, b"fixture").expect("write package");
        let list = source_list("https://example.com/index.json", SourceListType::LnReader);
        let candidate =
            candidates_from_caches(vec![cache(&list, 0, "fixture", serde_json::json!("1.2.3"))])
                .remove(0);

        crate::source::Source::write_meta_file(&package, candidate.provenance(), None)
            .expect("write source provenance");
        let meta_path = crate::source::BlockingSource::meta_source_path(&package)
            .expect("resolve metadata path");
        let meta: crate::source::SourceMeta =
            serde_json::from_slice(&fs::read(meta_path).expect("read source provenance"))
                .expect("parse source provenance");

        assert_eq!(
            meta.catalog_list_id.as_deref(),
            Some(candidate.list_id.as_str())
        );
        assert_eq!(meta.provider_url.as_ref(), Some(&candidate.provider_url));
        assert_eq!(meta.installed_version, Some(serde_json::json!("1.2.3")));
    }

    #[test]
    fn a_corrupt_cache_does_not_hide_another_valid_list() {
        let directory = tempdir().expect("create cache directory");
        let store = CatalogStore::new(directory.path().to_path_buf());
        let valid = source_list("https://valid.example/index.json", SourceListType::LnReader);
        let corrupt = source_list(
            "https://broken.example/index.json",
            SourceListType::LnReader,
        );
        let valid_cache = cache(&valid, 0, "fixture", serde_json::json!("1.2.3"));
        store.save_cache(&valid_cache).expect("save valid cache");
        fs::write(store.cache_path(&source_list_id(&corrupt)), b"{").expect("write corrupt cache");

        let candidates = store.load(&[valid, corrupt]).expect("load active caches");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source.id.value(), "fixture");
    }
}
