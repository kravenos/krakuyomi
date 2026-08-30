use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;

use anyhow::{bail, Context, Result};

use crate::{
    model::SourceId,
    settings::{Settings, SourceSettingValue},
    source::{Source, SourceBackend},
    source_collection::SourceCollection,
};

/// A supported source package format found in the local sources directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourcePackageKind {
    /// Aidoku WASM archive (`.aix`).
    Aidoku,
    /// Mihon/Keiyoushi extension archive (`.keiyoushi.apk`).
    Keiyoushi,
    /// LNReader JavaScript plugin (`.lnreader.js`).
    LnReader,
    /// MangaYomi Dart or JavaScript extension.
    MangaYomi,
}

impl SourcePackageKind {
    /// Returns the stable API name for this package kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Aidoku => "aidoku",
            Self::Keiyoushi => "keiyoushi",
            Self::LnReader => "lnreader",
            Self::MangaYomi => "mangayomi",
        }
    }
}

/// The result of attempting to load one local source package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourcePackageLoadState {
    /// The package loaded and registered at least one source.
    Loaded,
    /// The package could not be loaded or conflicted with an earlier package.
    LoadFailed,
}

impl SourcePackageLoadState {
    /// Returns the stable API name for this load state.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Loaded => "loaded",
            Self::LoadFailed => "load_failed",
        }
    }
}

/// A path-safe summary of one supported package discovered during source loading.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePackageStatus {
    /// File name only; never the full local filesystem path.
    pub package_label: String,
    /// Package format detected from the file name.
    pub kind: SourcePackageKind,
    /// Source ids registered by the package, or a best-effort filename hint.
    pub source_ids: Vec<SourceId>,
    /// Whether the package loaded successfully.
    pub load: SourcePackageLoadState,
    /// Sanitized user-facing failure summary, when loading failed.
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct SourceManager {
    sources_folder: PathBuf,
    pub sources_by_id: HashMap<SourceId, Source>,
    pub settings: Settings,
    /// Maps every loaded source id to its backing package path.
    pub file_sources: HashMap<String, String>,
    /// Stable, path-safe result for every supported package in the last scan.
    pub source_packages: Vec<SourcePackageStatus>,
}

impl SourceManager {
    pub fn new(
        sources_folder: PathBuf,
        sources_by_id: HashMap<SourceId, Source>,
        settings: Settings,
    ) -> Self {
        Self {
            sources_folder,
            sources_by_id,
            settings,
            file_sources: HashMap::new(),
            source_packages: Vec::new(),
        }
    }

    pub fn from_folder(path: PathBuf, settings: Settings) -> Result<Self> {
        fs::create_dir_all(&path).context("while trying to ensure sources folder exists")?;

        Ok(Self {
            sources_folder: path,
            sources_by_id: HashMap::new(),
            settings,
            file_sources: HashMap::new(),
            source_packages: Vec::new(),
        })
    }

    pub fn install_source(
        &mut self,
        id: &SourceId,
        contents: impl AsRef<[u8]>,
        source_of_source: String,
        arc_manager: &Arc<Mutex<SourceManager>>,
    ) -> Result<()> {
        let target_path = self.source_path(id);
        fs::write(&target_path, contents)?;

        Source::write_meta_file(&target_path, source_of_source, None)?;

        let source = Source::from_aix_file(&target_path, self, arc_manager)?;
        self.sources_by_id.insert(id.clone(), source);
        self.file_sources.insert(
            id.value().to_owned(),
            target_path.to_string_lossy().to_string(),
        );
        self.record_loaded_package(&target_path, vec![id.clone()]);

        Ok(())
    }

    /// Installs an LNReader plugin: the raw JS is stored as `<id>.lnreader.js`.
    pub fn install_lnreader_source(
        &mut self,
        id: &SourceId,
        contents: impl AsRef<[u8]>,
        source_of_source: String,
        arc_manager: &Arc<Mutex<SourceManager>>,
    ) -> Result<()> {
        let target_path = self.lnreader_source_path(id);
        fs::write(&target_path, contents)?;

        Source::write_meta_file(&target_path, source_of_source, None)?;

        let source = Source::from_lnreader_file(&target_path, self, arc_manager)?;
        // Installing is an explicit user action with the network up, so the
        // probe runs right away: it writes the probe cache (later loads read
        // it and skip the JS evaluation) and the source is fully probed from
        // the start, showing its real manifest in the installed-sources list.
        if let Err(e) = source
            .probe()
            .with_context(|| format!("failed to probe LNReader plugin {}", id.value()))
        {
            // Probe failed: remove the plugin and metadata files to avoid
            // leaving a partially installed source on disk.
            let _ = fs::remove_file(&target_path);
            if let Ok(meta_path) = crate::source::BlockingSource::meta_source_path(&target_path) {
                let _ = fs::remove_file(&meta_path);
            }
            let probe_path = self.lnreader_probe_path(id);
            let _ = fs::remove_file(&probe_path);
            return Err(e);
        }
        self.sources_by_id.insert(id.clone(), source);
        self.file_sources.insert(
            id.value().to_owned(),
            target_path.to_string_lossy().to_string(),
        );
        self.record_loaded_package(&target_path, vec![id.clone()]);

        Ok(())
    }

    /// Installs a MangaYomi extension: the code is stored as
    /// `<id>.mangayomi.dart` or `<id>.mangayomi.js` (per the
    /// `sourceCodeLanguage` field of the index entry: `0` Dart, `1`
    /// JavaScript) with its `index.json` entry as a `<id>.mangayomi.json`
    /// sidecar. Anime extensions (`itemType: 1`) are rejected.
    pub fn install_mangayomi_source(
        &mut self,
        id: &SourceId,
        code: impl AsRef<[u8]>,
        metadata: impl AsRef<[u8]>,
        source_of_source: String,
        arc_manager: &Arc<Mutex<SourceManager>>,
    ) -> Result<()> {
        let metadata: serde_json::Value =
            serde_json::from_slice(metadata.as_ref()).context("invalid extension metadata JSON")?;
        if metadata
            .get("itemType")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 1
        {
            bail!(
                "MangaYomi anime extension '{}' is not supported",
                id.value()
            );
        }
        // The stored metadata must carry its own `id`; `from_mangayomi_file`
        // rejects metadata without one. The install pipelines that lose the
        // key (e.g. `#[serde(flatten)]` in `install_source`) restore it before
        // calling this.
        let metadata: serde_json::Value = match metadata.get("id") {
            Some(_) => metadata,
            None => bail!(
                "MangaYomi extension metadata for '{}' is missing its `id`",
                id.value()
            ),
        };
        let is_js = metadata
            .get("sourceCodeLanguage")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 1;
        let target_path = if is_js {
            self.mangayomi_js_source_path(id)
        } else {
            self.mangayomi_source_path(id)
        };
        fs::write(&target_path, code)?;
        fs::write(target_path.with_extension("json"), metadata.to_string())?;

        Source::write_meta_file(&target_path, source_of_source, None)?;

        let source = Source::from_mangayomi_file(&target_path, self, arc_manager)?;
        // See `install_lnreader_source`: the probe runs eagerly so the probe
        // cache is written and the source is fully probed from the start.
        if let Err(e) = source
            .probe()
            .with_context(|| format!("failed to probe MangaYomi extension {}", id.value()))
        {
            // Probe failed: remove the extension, metadata, and meta files to
            // avoid leaving a partially installed source on disk.
            let _ = fs::remove_file(&target_path);
            let _ = fs::remove_file(target_path.with_extension("json"));
            if let Ok(meta_path) = crate::source::BlockingSource::meta_source_path(&target_path) {
                let _ = fs::remove_file(&meta_path);
            }
            let probe_path = self.mangayomi_probe_path(id);
            let _ = fs::remove_file(&probe_path);
            return Err(e);
        }
        self.sources_by_id.insert(id.clone(), source);
        self.file_sources.insert(
            id.value().to_owned(),
            target_path.to_string_lossy().to_string(),
        );
        self.record_loaded_package(&target_path, vec![id.clone()]);

        Ok(())
    }

    /// Installs a keiyoushi extension APK: the bytes are stored as
    /// `<pkg>.keiyoushi.apk` (one per extension package), and every source
    /// bundled in the APK is registered individually (see
    /// [`Source::from_keiyoushi_file`]). For a multi-source APK,
    /// `languages` restricts which bundled sources are registered; the
    /// selection is stored in the meta file so reloads keep it.
    pub fn install_keiyoushi_source(
        &mut self,
        id: &SourceId,
        contents: impl AsRef<[u8]>,
        source_of_source: String,
        arc_manager: &Arc<Mutex<SourceManager>>,
        languages: Option<&[String]>,
    ) -> Result<()> {
        if languages.is_some_and(|langs| langs.is_empty()) {
            bail!("keiyoushi language selection is empty");
        }

        let target_path = self.keiyoushi_source_path(id);
        fs::write(&target_path, contents)?;

        Source::write_meta_file(
            &target_path,
            source_of_source,
            languages.map(|l| l.to_vec()),
        )?;

        // The selection in the meta file replaces any previous one, so drop
        // the sources previously registered from this APK first; otherwise
        // the registered set would mix the old and the new selection.
        let doomed: Vec<SourceId> = self
            .sources_by_id
            .iter()
            .filter_map(|(id, source)| match &source.backend {
                SourceBackend::Keiyoushi(keiyoushi) => {
                    (keiyoushi.apk_path() == target_path).then(|| id.clone())
                }
                _ => None,
            })
            .collect();
        for doomed_id in doomed {
            self.sources_by_id.remove(&doomed_id);
            self.file_sources.remove(doomed_id.value());
        }

        let sources = Source::from_keiyoushi_file(&target_path, self, arc_manager)?;
        let mut registered_ids = Vec::new();
        for source in sources {
            let source_id = SourceId::new(source.manifest().info.id.clone());
            self.file_sources.insert(
                source_id.value().to_owned(),
                target_path.to_string_lossy().to_string(),
            );
            registered_ids.push(source_id.clone());
            self.sources_by_id.insert(source_id, source);
        }
        self.record_loaded_package(&target_path, registered_ids);

        Ok(())
    }

    pub fn uninstall_source(&mut self, id: &SourceId) -> Result<()> {
        let source_path = self.source_path(id);
        fs::remove_file(&source_path)?;

        self.sources_by_id.remove(&id.clone());
        self.file_sources.remove(id.value());
        self.remove_package_statuses_for_paths(std::slice::from_ref(&source_path));

        Ok(())
    }

    /// Removes a WASM, an LNReader, a MangaYomi and a Keiyoushi source file
    /// if present. Keiyoushi sources of the same APK share one file, so the
    /// removal clears every registered source of that extension.
    pub fn uninstall_any_source(&mut self, id: &SourceId) -> Result<()> {
        let affected_source_ids = self.package_source_ids(id)?;
        let mut package_paths = vec![
            self.source_path(id),
            self.lnreader_source_path(id),
            self.lnreader_probe_path(id),
            self.mangayomi_source_path(id),
            self.mangayomi_js_source_path(id),
            self.mangayomi_probe_path(id),
            self.keiyoushi_source_path(id),
            self.keiyoushi_probe_path(id),
        ];
        if let Some(loaded_path) = self.source_file_for_id(id) {
            if loaded_path.parent() != Some(self.sources_folder.as_path()) {
                bail!("loaded source path is outside the sources folder")
            }
            package_paths.push(loaded_path);
        }
        package_paths.sort();
        package_paths.dedup();
        let mut artifact_paths = Vec::new();
        for path in &package_paths {
            artifact_paths.push(path.clone());
            artifact_paths.push(path.with_extension("json"));
            if let Ok(meta_path) = crate::source::BlockingSource::meta_source_path(path) {
                artifact_paths.push(meta_path);
            }
        }
        artifact_paths.sort();
        artifact_paths.dedup();

        for path in &artifact_paths {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    bail!(
                        "couldn't remove source artifact '{}': {error}",
                        source_file_label(path)
                    )
                }
            }
        }
        if let Some(remaining) = artifact_paths.iter().find(|path| path.exists()) {
            bail!(
                "source artifact '{}' remains after uninstall",
                source_file_label(remaining)
            );
        }

        for source_id in affected_source_ids {
            self.sources_by_id.remove(&source_id);
            self.file_sources.remove(source_id.value());
        }
        self.remove_package_statuses_for_paths(&package_paths);
        Ok(())
    }

    /// Returns every loaded source id removed when `id` is uninstalled.
    /// Multi-source Keiyoushi extensions share one package and are returned
    /// together so previews can count all affected library manga.
    pub fn package_source_ids(&self, id: &SourceId) -> Result<Vec<SourceId>> {
        validate_source_id(id)?;
        let package_path = self.keiyoushi_source_path(id);
        let mut source_ids = self
            .sources_by_id
            .iter()
            .filter_map(|(source_id, source)| match &source.backend {
                SourceBackend::Keiyoushi(source) if source.apk_path() == package_path => {
                    Some(source_id.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if source_ids.is_empty() {
            source_ids.push(id.clone());
        }
        source_ids.sort_by(|left, right| left.value().cmp(right.value()));
        Ok(source_ids)
    }

    pub fn update_settings(
        &mut self,
        settings: Settings,
        manager: &Arc<Mutex<SourceManager>>,
    ) -> Result<()> {
        // Only the per-source stored settings affect the loaded sources;
        // global settings (source lists, languages, ...) must not tear down
        // every extension. Reload just the files backing the sources whose
        // settings changed, instead of re-scanning and re-probing the whole
        // collection.
        let changed = self.changed_source_ids(&settings);
        self.settings = settings;
        if changed.is_empty() {
            return Ok(());
        }

        // Several sources may share one file (a keiyoushi APK registers one
        // source per bundled `Source`), so dedupe the affected files.
        let mut files = BTreeSet::new();
        for id in &changed {
            if let Some(path) = self.source_file_for_id(id) {
                files.insert(path);
            }
        }
        for path in files {
            self.reload_source_file(&path, manager)?;
        }

        Ok(())
    }

    /// The ids of the sources whose stored settings differ between the
    /// current settings and the given one.
    fn changed_source_ids(&self, settings: &Settings) -> Vec<SourceId> {
        let old = &self.settings.source_settings;
        let new = &settings.source_settings;
        let mut keys: Vec<&String> = old.keys().collect();
        keys.extend(new.keys());
        keys.sort();
        keys.dedup();
        keys.into_iter()
            .filter(|key| old.get(*key) != new.get(*key))
            .map(|key| SourceId::new(key.clone()))
            .collect()
    }

    /// The on-disk file a registered source was loaded from, if any.
    fn source_file_for_id(&self, id: &SourceId) -> Option<PathBuf> {
        if let Some(path) = self.file_sources.get(id.value()) {
            return Some(PathBuf::from(path));
        }
        let candidates = match self.sources_by_id.get(id).map(|source| &source.backend) {
            Some(SourceBackend::Keiyoushi(keiyoushi)) => {
                vec![keiyoushi.apk_path().to_path_buf()]
            }
            _ => vec![],
        };
        candidates
            .into_iter()
            .chain([
                self.lnreader_source_path(id),
                self.mangayomi_source_path(id),
                self.mangayomi_js_source_path(id),
                self.keiyoushi_source_path(id),
            ])
            .find(|path| path.exists())
    }

    /// Drop every source registered from `path`, then re-register them from
    /// the file. Re-running the loader picks up the freshly saved stored
    /// settings, and dropping the old sources tears down their worker
    /// engines so the next call boots with the new values.
    fn reload_source_file(
        &mut self,
        path: &Path,
        manager: &Arc<Mutex<SourceManager>>,
    ) -> Result<()> {
        let doomed: Vec<SourceId> = self
            .sources_by_id
            .keys()
            .filter(|id| self.source_file_for_id(id).as_deref() == Some(path))
            .cloned()
            .collect();
        for id in &doomed {
            self.sources_by_id.remove(id);
            self.file_sources.remove(id.value());
        }

        let name = path.file_name().map(|n| n.to_string_lossy().to_string());
        let is_keiyoushi = name
            .as_deref()
            .is_some_and(|name| name.ends_with(crate::source::keiyoushi::KEIYOUSHI_FILE_SUFFIX));
        let is_lnreader = name
            .as_deref()
            .is_some_and(|name| name.ends_with(crate::source::lnreader::LNREADER_FILE_SUFFIX));
        let is_mangayomi = name.as_deref().is_some_and(|name| {
            name.ends_with(crate::source::mangayomi::MANGA_YOMI_FILE_SUFFIX)
                || name.ends_with(crate::source::mangayomi::MANGA_YOMI_JS_FILE_SUFFIX)
        });

        let load_result = if is_keiyoushi {
            Source::from_keiyoushi_file(path, self, manager)
        } else if is_lnreader {
            Source::from_lnreader_file(path, self, manager).map(|source| vec![source])
        } else if is_mangayomi {
            Source::from_mangayomi_file(path, self, manager).map(|source| vec![source])
        } else {
            Source::from_aix_file(path, self, manager).map(|source| vec![source])
        };
        let sources = match load_result {
            Ok(sources) => sources,
            Err(error) => {
                log::warn!(
                    "couldn't reload source package {}: {error:#}",
                    path.display()
                );
                self.record_failed_package(path, doomed);
                return Err(error);
            }
        };

        for source in sources {
            let id = source.manifest().info.id.clone();
            self.file_sources
                .insert(id.clone(), path.to_string_lossy().to_string());
            self.sources_by_id.insert(SourceId::new(id.clone()), source);
        }

        let source_ids = self
            .sources_by_id
            .keys()
            .filter(|id| self.source_file_for_id(id).as_deref() == Some(path))
            .cloned()
            .collect();
        self.record_loaded_package(path, source_ids);

        Ok(())
    }

    pub fn update_source_setting(
        &mut self,
        source_id: String,
        snapshot: HashMap<String, SourceSettingValue>,
        arc_manager: &Arc<Mutex<SourceManager>>,
    ) -> Result<()> {
        let mut settings = self.settings.clone();
        settings.source_settings.insert(source_id, snapshot);

        self.settings = settings;
        self.sources_by_id = self.load_all_sources(arc_manager)?;

        Ok(())
    }

    pub fn load_all_sources(
        &mut self,
        manager: &Arc<Mutex<SourceManager>>,
    ) -> Result<HashMap<SourceId, Source>> {
        let entries = fs::read_dir(&self.sources_folder).with_context(|| {
            format!(
                "while attempting to read source collection at {}",
                self.sources_folder.display()
            )
        })?;

        let mut package_paths = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) if source_package_kind(&entry.path()).is_some() => {
                    package_paths.push(entry.path());
                }
                Ok(_) => {}
                Err(error) => {
                    log::warn!("couldn't inspect an entry in the sources folder: {error}");
                }
            }
        }
        package_paths.sort_by_key(|path| {
            let label = source_package_label(path);
            (label.to_ascii_lowercase(), label)
        });

        let mut sources_by_id = HashMap::new();
        let mut file_sources = HashMap::new();
        let mut source_packages = Vec::new();
        for path in package_paths {
            let kind = source_package_kind(&path).expect("supported package path");
            let package_label = source_package_label(&path);
            let sources = match self.load_source_package(&path, kind, manager) {
                Ok(sources) => sources,
                Err(error) => {
                    log::warn!("couldn't load source package {}: {error:#}", path.display());
                    source_packages.push(SourcePackageStatus {
                        package_label,
                        kind,
                        source_ids: source_id_hint(&path, kind).into_iter().collect(),
                        load: SourcePackageLoadState::LoadFailed,
                        error: Some("The source package is invalid or incompatible.".to_owned()),
                    });
                    continue;
                }
            };

            let source_ids = sources
                .iter()
                .map(|source| SourceId::new(source.manifest().info.id.clone()))
                .collect::<Vec<_>>();
            if source_ids.is_empty() {
                source_packages.push(SourcePackageStatus {
                    package_label,
                    kind,
                    source_ids: source_id_hint(&path, kind).into_iter().collect(),
                    load: SourcePackageLoadState::LoadFailed,
                    error: Some("The source package did not provide any sources.".to_owned()),
                });
                continue;
            }

            let mut ids_in_package = BTreeSet::new();
            let duplicate_id = source_ids
                .iter()
                .find(|id| {
                    !ids_in_package.insert(id.value().clone()) || sources_by_id.contains_key(*id)
                })
                .map(|id| id.value().clone());
            if let Some(duplicate_id) = duplicate_id {
                log::warn!(
                    "rejected source package {} because source id '{}' was already loaded",
                    path.display(),
                    duplicate_id
                );
                source_packages.push(SourcePackageStatus {
                    package_label,
                    kind,
                    source_ids,
                    load: SourcePackageLoadState::LoadFailed,
                    error: Some(format!(
                        "Source id '{}' conflicts with an earlier package.",
                        duplicate_id
                    )),
                });
                continue;
            }

            for (source_id, source) in source_ids.iter().cloned().zip(sources) {
                file_sources.insert(
                    source_id.value().clone(),
                    path.to_string_lossy().to_string(),
                );
                sources_by_id.insert(source_id, source);
            }
            source_packages.push(SourcePackageStatus {
                package_label,
                kind,
                source_ids,
                load: SourcePackageLoadState::Loaded,
                error: None,
            });
        }

        self.file_sources = file_sources;
        self.source_packages = source_packages;

        Ok(sources_by_id)
    }

    fn load_source_package(
        &self,
        path: &Path,
        kind: SourcePackageKind,
        manager: &Arc<Mutex<SourceManager>>,
    ) -> Result<Vec<Source>> {
        match kind {
            SourcePackageKind::Aidoku => Ok(vec![Source::from_aix_file(path, self, manager)?]),
            SourcePackageKind::Keiyoushi => Source::from_keiyoushi_file(path, self, manager),
            SourcePackageKind::LnReader => {
                Ok(vec![Source::from_lnreader_file(path, self, manager)?])
            }
            SourcePackageKind::MangaYomi => {
                Ok(vec![Source::from_mangayomi_file(path, self, manager)?])
            }
        }
    }

    fn record_loaded_package(&mut self, path: &Path, source_ids: Vec<SourceId>) {
        let Some(kind) = source_package_kind(path) else {
            return;
        };
        let package_label = source_package_label(path);
        self.source_packages
            .retain(|status| status.package_label != package_label);
        self.source_packages.push(SourcePackageStatus {
            package_label,
            kind,
            source_ids,
            load: SourcePackageLoadState::Loaded,
            error: None,
        });
        self.sort_package_statuses();
    }

    fn record_failed_package(&mut self, path: &Path, mut source_ids: Vec<SourceId>) {
        let Some(kind) = source_package_kind(path) else {
            return;
        };
        if source_ids.is_empty() {
            source_ids.extend(source_id_hint(path, kind));
        }
        let package_label = source_package_label(path);
        self.source_packages
            .retain(|status| status.package_label != package_label);
        self.source_packages.push(SourcePackageStatus {
            package_label,
            kind,
            source_ids,
            load: SourcePackageLoadState::LoadFailed,
            error: Some("The source package is invalid or incompatible.".to_owned()),
        });
        self.sort_package_statuses();
    }

    fn sort_package_statuses(&mut self) {
        self.source_packages.sort_by_key(|status| {
            (
                status.package_label.to_ascii_lowercase(),
                status.package_label.clone(),
            )
        });
    }

    fn remove_package_statuses_for_paths<'a>(
        &mut self,
        paths: impl IntoIterator<Item = &'a PathBuf>,
    ) {
        let labels = paths
            .into_iter()
            .map(|path| source_package_label(path))
            .collect::<BTreeSet<_>>();
        self.source_packages
            .retain(|status| !labels.contains(&status.package_label));
    }

    pub fn source_path(&self, id: &SourceId) -> PathBuf {
        self.sources_folder.join(format!("{}.aix", id.value()))
    }

    pub fn lnreader_source_path(&self, id: &SourceId) -> PathBuf {
        self.sources_folder.join(format!(
            "{}{}",
            id.value(),
            crate::source::lnreader::LNREADER_FILE_SUFFIX
        ))
    }

    /// The path of the probe cache sidecar of an LNReader plugin.
    pub fn lnreader_probe_path(&self, id: &SourceId) -> PathBuf {
        self.sources_folder.join(format!(
            "{}{}",
            id.value(),
            crate::source::lnreader::LNREADER_PROBE_SUFFIX
        ))
    }

    pub fn mangayomi_source_path(&self, id: &SourceId) -> PathBuf {
        self.sources_folder.join(format!(
            "{}{}",
            id.value(),
            crate::source::mangayomi::MANGA_YOMI_FILE_SUFFIX
        ))
    }

    pub fn mangayomi_js_source_path(&self, id: &SourceId) -> PathBuf {
        self.sources_folder.join(format!(
            "{}{}",
            id.value(),
            crate::source::mangayomi::MANGA_YOMI_JS_FILE_SUFFIX
        ))
    }

    /// The path of the probe cache sidecar of a MangaYomi extension.
    pub fn mangayomi_probe_path(&self, id: &SourceId) -> PathBuf {
        self.sources_folder.join(format!(
            "{}{}",
            id.value(),
            crate::source::mangayomi::MANGA_YOMI_PROBE_SUFFIX
        ))
    }

    /// The path of the keiyoushi extension APK of a source id. Multi-source
    /// APKs register their sources as `<pkg>:<lang>`; they all share the
    /// `<pkg>.keiyoushi.apk` file.
    pub fn keiyoushi_source_path(&self, id: &SourceId) -> PathBuf {
        let pkg = id.value().split(':').next().unwrap_or(id.value());
        self.sources_folder.join(format!(
            "{pkg}{}",
            crate::source::keiyoushi::KEIYOUSHI_FILE_SUFFIX
        ))
    }

    /// The path of the probe cache sidecar of a keiyoushi extension APK
    /// (one per extension package).
    pub fn keiyoushi_probe_path(&self, id: &SourceId) -> PathBuf {
        let pkg = id.value().split(':').next().unwrap_or(id.value());
        self.sources_folder.join(format!(
            "{pkg}{}",
            crate::source::keiyoushi::KEIYOUSHI_PROBE_SUFFIX
        ))
    }
}

fn source_package_kind(path: &Path) -> Option<SourcePackageKind> {
    let label = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if label.ends_with(crate::source::keiyoushi::KEIYOUSHI_FILE_SUFFIX) {
        Some(SourcePackageKind::Keiyoushi)
    } else if label.ends_with(crate::source::lnreader::LNREADER_FILE_SUFFIX) {
        Some(SourcePackageKind::LnReader)
    } else if label.ends_with(crate::source::mangayomi::MANGA_YOMI_FILE_SUFFIX)
        || label.ends_with(crate::source::mangayomi::MANGA_YOMI_JS_FILE_SUFFIX)
    {
        Some(SourcePackageKind::MangaYomi)
    } else if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("aix"))
    {
        Some(SourcePackageKind::Aidoku)
    } else {
        None
    }
}

fn source_package_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown source package".to_owned())
}

fn source_id_hint(path: &Path, kind: SourcePackageKind) -> Option<SourceId> {
    let label = path.file_name()?.to_string_lossy();
    let id = match kind {
        SourcePackageKind::Aidoku => path.file_stem()?.to_string_lossy().to_string(),
        SourcePackageKind::Keiyoushi => label
            .strip_suffix(crate::source::keiyoushi::KEIYOUSHI_FILE_SUFFIX)?
            .to_owned(),
        SourcePackageKind::LnReader => label
            .strip_suffix(crate::source::lnreader::LNREADER_FILE_SUFFIX)?
            .to_owned(),
        SourcePackageKind::MangaYomi => label
            .strip_suffix(crate::source::mangayomi::MANGA_YOMI_FILE_SUFFIX)
            .or_else(|| label.strip_suffix(crate::source::mangayomi::MANGA_YOMI_JS_FILE_SUFFIX))?
            .to_owned(),
    };
    Some(SourceId::new(id))
}

fn source_file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown source file".to_owned())
}

fn validate_source_id(id: &SourceId) -> Result<()> {
    let value = id.value();
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
    {
        bail!("invalid source id")
    }
    Ok(())
}

impl SourceCollection for SourceManager {
    fn get_by_id(&self, id: &SourceId) -> Option<&Source> {
        self.sources_by_id.get(id)
    }

    fn sources(&self) -> Vec<&Source> {
        self.sources_by_id.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::tempdir;
    use zip::{write::FileOptions, ZipWriter};

    use super::*;

    fn write_test_aix(path: &Path, source_id: &str, name: &str) {
        let file = fs::File::create(path).expect("create test source archive");
        let mut archive = ZipWriter::new(file);
        let options: FileOptions<'_, ()> = FileOptions::default();
        archive
            .start_file("Payload/source.json", options)
            .expect("start source manifest");
        write!(
            archive,
            r#"{{"info":{{"id":"{source_id}","name":"{name}","version":1}}}}"#
        )
        .expect("write source manifest");
        archive.finish().expect("finish test source archive");
    }

    #[test]
    fn corrupt_package_does_not_hide_later_valid_package() {
        let directory = tempdir().expect("create sources directory");
        fs::write(directory.path().join("00-broken.aix"), b"not a zip")
            .expect("write corrupt source");
        write_test_aix(
            &directory.path().join("10-valid.aix"),
            "valid.source",
            "Valid Source",
        );
        let manager = Arc::new(Mutex::new(
            SourceManager::from_folder(directory.path().to_path_buf(), Settings::default())
                .expect("create source manager"),
        ));

        let mut guard = manager.blocking_lock();
        let sources = guard
            .load_all_sources(&manager)
            .expect("scan should survive one corrupt package");

        assert!(sources.contains_key(&SourceId::new("valid.source".to_owned())));
        assert_eq!(guard.source_packages.len(), 2);
        assert_eq!(
            guard.source_packages[0].load,
            SourcePackageLoadState::LoadFailed
        );
        assert_eq!(guard.source_packages[0].package_label, "00-broken.aix");
        assert_eq!(
            guard.source_packages[1].load,
            SourcePackageLoadState::Loaded
        );
    }

    #[test]
    fn stable_package_order_rejects_later_duplicate_source_id() {
        let directory = tempdir().expect("create sources directory");
        write_test_aix(
            &directory.path().join("b-second.aix"),
            "duplicate.source",
            "Second",
        );
        write_test_aix(
            &directory.path().join("a-first.aix"),
            "duplicate.source",
            "First",
        );
        let manager = Arc::new(Mutex::new(
            SourceManager::from_folder(directory.path().to_path_buf(), Settings::default())
                .expect("create source manager"),
        ));

        let mut guard = manager.blocking_lock();
        let sources = guard.load_all_sources(&manager).expect("scan sources");
        let source = sources
            .get(&SourceId::new("duplicate.source".to_owned()))
            .expect("first source should win");

        assert_eq!(source.manifest().info.name, "First");
        assert_eq!(guard.source_packages[0].package_label, "a-first.aix");
        assert_eq!(
            guard.source_packages[0].load,
            SourcePackageLoadState::Loaded
        );
        assert_eq!(guard.source_packages[1].package_label, "b-second.aix");
        assert_eq!(
            guard.source_packages[1].load,
            SourcePackageLoadState::LoadFailed
        );
    }

    #[test]
    fn failed_reload_replaces_loaded_status_with_failure() {
        let directory = tempdir().expect("create sources directory");
        let package_path = directory.path().join("source.aix");
        write_test_aix(&package_path, "reload.source", "Reload Source");
        let manager = Arc::new(Mutex::new(
            SourceManager::from_folder(directory.path().to_path_buf(), Settings::default())
                .expect("create source manager"),
        ));

        let mut guard = manager.blocking_lock();
        let sources = guard.load_all_sources(&manager).expect("load source");
        guard.sources_by_id = sources;
        fs::write(&package_path, b"not a zip").expect("corrupt source package");

        let result = guard.reload_source_file(&package_path, &manager);

        assert!(result.is_err());
        assert!(!guard
            .sources_by_id
            .contains_key(&SourceId::new("reload.source".to_owned())));
        assert_eq!(guard.source_packages.len(), 1);
        assert_eq!(
            guard.source_packages[0].load,
            SourcePackageLoadState::LoadFailed
        );
        assert_eq!(
            guard.source_packages[0].source_ids,
            vec![SourceId::new("reload.source".to_owned())]
        );
    }

    #[test]
    fn uninstall_removes_and_verifies_every_known_artifact() {
        let directory = tempdir().expect("create sources directory");
        let mut manager =
            SourceManager::from_folder(directory.path().to_path_buf(), Settings::default())
                .expect("create source manager");
        let source_id = SourceId::new("test.source".to_owned());
        let package_paths = [
            manager.source_path(&source_id),
            manager.lnreader_source_path(&source_id),
            manager.lnreader_probe_path(&source_id),
            manager.mangayomi_source_path(&source_id),
            manager.mangayomi_js_source_path(&source_id),
            manager.mangayomi_probe_path(&source_id),
            manager.keiyoushi_source_path(&source_id),
            manager.keiyoushi_probe_path(&source_id),
        ];
        let mut artifact_paths = Vec::new();
        for path in package_paths {
            artifact_paths.push(path.clone());
            artifact_paths.push(path.with_extension("json"));
            artifact_paths.push(
                crate::source::BlockingSource::meta_source_path(&path)
                    .expect("derive metadata path"),
            );
        }
        let loaded_path = directory.path().join("renamed-package.aix");
        manager.file_sources.insert(
            source_id.value().clone(),
            loaded_path.to_string_lossy().into_owned(),
        );
        artifact_paths.push(loaded_path.clone());
        artifact_paths.push(
            crate::source::BlockingSource::meta_source_path(&loaded_path)
                .expect("derive loaded package metadata path"),
        );
        artifact_paths.sort();
        artifact_paths.dedup();
        for path in &artifact_paths {
            fs::write(path, b"test artifact").expect("write source artifact");
        }

        manager
            .uninstall_any_source(&source_id)
            .expect("remove source artifacts");

        assert!(artifact_paths.iter().all(|path| !path.exists()));
    }

    #[test]
    fn uninstall_rejects_path_traversal_source_id() {
        let directory = tempdir().expect("create sources directory");
        let outside = directory.path().join("outside.aix");
        fs::write(&outside, b"must remain").expect("write outside sentinel");
        let sources = directory.path().join("sources");
        let mut manager = SourceManager::from_folder(sources, Settings::default())
            .expect("create source manager");

        let result = manager.uninstall_any_source(&SourceId::new("..\\outside".to_owned()));

        assert!(result.is_err());
        assert_eq!(fs::read(outside).expect("read sentinel"), b"must remain");
    }

    #[test]
    fn uninstall_rejects_loaded_path_outside_sources_folder() {
        let directory = tempdir().expect("create test directory");
        let outside = directory.path().join("outside.aix");
        fs::write(&outside, b"must remain").expect("write outside sentinel");
        let mut manager =
            SourceManager::from_folder(directory.path().join("sources"), Settings::default())
                .expect("create source manager");
        let source_id = SourceId::new("safe.source".to_owned());
        manager.file_sources.insert(
            source_id.value().clone(),
            outside.to_string_lossy().into_owned(),
        );

        let result = manager.uninstall_any_source(&source_id);

        assert!(result.is_err());
        assert_eq!(fs::read(outside).expect("read sentinel"), b"must remain");
    }
}
