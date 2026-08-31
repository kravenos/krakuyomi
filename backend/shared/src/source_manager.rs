use std::{
    collections::{BTreeSet, HashMap},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;

use anyhow::{bail, Context, Result};
use tempfile::{Builder, TempDir};

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

/// Keeps the currently installed package recoverable while a staged
/// replacement is validated and published on the same filesystem.
struct SourceInstallTransaction {
    sources_folder: PathBuf,
    temporary: TempDir,
    targets: Vec<PathBuf>,
    backed_up: Vec<PathBuf>,
    installed: Vec<PathBuf>,
    rollback_required: bool,
    publish_complete: bool,
    committed: bool,
}

impl SourceInstallTransaction {
    fn new(sources_folder: &Path, mut targets: Vec<PathBuf>) -> Result<Self> {
        targets.sort();
        targets.dedup();
        for target in &targets {
            if target.parent() != Some(sources_folder) || target.file_name().is_none() {
                bail!("source install target is outside the sources folder")
            }
        }
        let temporary = Builder::new()
            .prefix(".rakuyomi-install-")
            .tempdir_in(sources_folder)
            .context("couldn't create source install staging directory")?;
        Ok(Self {
            sources_folder: sources_folder.to_path_buf(),
            temporary,
            targets,
            backed_up: Vec::new(),
            installed: Vec::new(),
            rollback_required: false,
            publish_complete: false,
            committed: false,
        })
    }

    fn staged_path(&self, target: &Path) -> Result<PathBuf> {
        let file_name = target
            .file_name()
            .context("source install target has no file name")?;
        Ok(self.temporary.path().join(file_name))
    }

    fn write(&self, target: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
        let staged = self.staged_path(target)?;
        let mut file = fs::File::create(&staged).with_context(|| {
            format!(
                "couldn't stage source artifact '{}'",
                source_file_label(target)
            )
        })?;
        file.write_all(contents.as_ref()).with_context(|| {
            format!(
                "couldn't write source artifact '{}'",
                source_file_label(target)
            )
        })?;
        file.sync_all().with_context(|| {
            format!(
                "couldn't sync source artifact '{}'",
                source_file_label(target)
            )
        })?;
        Ok(())
    }

    fn publish(&mut self) -> Result<()> {
        let result = self.publish_inner();
        if let Err(error) = result {
            return match self.rollback() {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(anyhow::anyhow!(
                    "{error:#}; restoring the prior source package also failed: {rollback_error:#}"
                )),
            };
        }
        self.publish_complete = true;
        sync_source_directory(&self.sources_folder);
        Ok(())
    }

    fn publish_inner(&mut self) -> Result<()> {
        let rollback_folder = self.temporary.path().join("rollback");
        fs::create_dir(&rollback_folder).context("couldn't create source rollback directory")?;
        self.rollback_required = true;

        for target in &self.targets {
            if target.exists() {
                let backup = rollback_folder.join(
                    target
                        .file_name()
                        .context("source install target has no file name")?,
                );
                fs::rename(target, &backup).with_context(|| {
                    format!(
                        "couldn't retain prior source artifact '{}'",
                        source_file_label(target)
                    )
                })?;
                self.backed_up.push(target.clone());
            }
        }

        for target in &self.targets {
            let staged = self.staged_path(target)?;
            if staged.exists() {
                fs::rename(&staged, target).with_context(|| {
                    format!(
                        "couldn't publish source artifact '{}'",
                        source_file_label(target)
                    )
                })?;
                self.installed.push(target.clone());
            }
        }
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        let rollback_folder = self.temporary.path().join("rollback");
        let mut failures = Vec::new();
        let mut installed = Vec::new();

        for target in self.installed.drain(..) {
            match fs::remove_file(&target) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    failures.push(format!(
                        "couldn't remove replacement '{}': {error}",
                        source_file_label(&target)
                    ));
                    installed.push(target);
                }
            }
        }
        self.installed = installed;

        let mut backed_up = Vec::new();
        while let Some(target) = self.backed_up.pop() {
            let Some(file_name) = target.file_name() else {
                failures.push("rollback target has no file name".to_owned());
                backed_up.push(target);
                continue;
            };
            let backup = rollback_folder.join(file_name);
            if !backup.exists() {
                continue;
            }
            if let Err(error) = fs::rename(&backup, &target) {
                failures.push(format!(
                    "couldn't restore prior artifact '{}': {error}",
                    source_file_label(&target)
                ));
                backed_up.push(target);
            }
        }
        backed_up.reverse();
        self.backed_up = backed_up;
        sync_source_directory(&self.sources_folder);

        if failures.is_empty() {
            self.publish_complete = false;
            self.rollback_required = false;
            self.installed.clear();
            self.backed_up.clear();
            Ok(())
        } else {
            bail!(failures.join("; "))
        }
    }

    fn rollback_error(&mut self, error: anyhow::Error) -> anyhow::Error {
        match self.rollback() {
            Ok(()) => error,
            Err(rollback_error) => anyhow::anyhow!(
                "{error:#}; restoring the prior source package also failed: {rollback_error:#}"
            ),
        }
    }

    fn commit(mut self) {
        debug_assert!(self.publish_complete);
        self.committed = true;
        sync_source_directory(&self.sources_folder);
    }
}

impl Drop for SourceInstallTransaction {
    fn drop(&mut self) {
        if self.rollback_required && !self.committed {
            if let Err(error) = self.rollback() {
                log::error!("failed to roll back source package replacement: {error:#}");
            }
        }
    }
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

    fn install_transaction(
        &self,
        id: &SourceId,
        target_path: &Path,
    ) -> Result<SourceInstallTransaction> {
        validate_source_id(id)?;
        let mut targets = source_package_artifact_paths(target_path)?;

        // A MangaYomi update may switch between Dart and JavaScript. Keep
        // both possible package paths in the rollback set so the obsolete
        // implementation cannot remain beside the verified replacement.
        if source_package_kind(target_path) == Some(SourcePackageKind::MangaYomi) {
            targets.extend(source_package_artifact_paths(
                &self.mangayomi_source_path(id),
            )?);
            targets.extend(source_package_artifact_paths(
                &self.mangayomi_js_source_path(id),
            )?);
        }

        if let Some(previous_path) = self.source_file_for_id(id) {
            if previous_path.parent() != Some(self.sources_folder.as_path()) {
                bail!("loaded source path is outside the sources folder")
            }
            targets.extend(source_package_artifact_paths(&previous_path)?);
        }

        SourceInstallTransaction::new(&self.sources_folder, targets)
    }

    pub fn install_source(
        &mut self,
        id: &SourceId,
        contents: impl AsRef<[u8]>,
        provenance: impl Into<crate::source_catalog::SourceProvenance>,
        arc_manager: &Arc<Mutex<SourceManager>>,
    ) -> Result<()> {
        let target_path = self.source_path(id);
        let mut transaction = self.install_transaction(id, &target_path)?;
        transaction.write(&target_path, contents)?;
        let staged_path = transaction.staged_path(&target_path)?;
        Source::write_meta_file(&staged_path, provenance, None)?;
        sync_staged_metadata(&transaction, &target_path)?;

        let staged_source = Source::from_aix_file(&staged_path, self, arc_manager)
            .context("staged Aidoku package did not load")?;
        validate_expected_sources(id, std::slice::from_ref(&staged_source))?;
        drop(staged_source);

        transaction.publish()?;
        let source = match Source::from_aix_file(&target_path, self, arc_manager)
            .context("published Aidoku package did not load")
        {
            Ok(source) => source,
            Err(error) => return Err(transaction.rollback_error(error)),
        };
        if let Err(error) = validate_expected_sources(id, std::slice::from_ref(&source)) {
            return Err(transaction.rollback_error(error));
        }
        self.sources_by_id.insert(id.clone(), source);
        self.file_sources.insert(
            id.value().to_owned(),
            target_path.to_string_lossy().to_string(),
        );
        self.record_loaded_package(&target_path, vec![id.clone()]);
        transaction.commit();

        Ok(())
    }

    /// Installs an LNReader plugin: the raw JS is stored as `<id>.lnreader.js`.
    pub fn install_lnreader_source(
        &mut self,
        id: &SourceId,
        contents: impl AsRef<[u8]>,
        provenance: impl Into<crate::source_catalog::SourceProvenance>,
        arc_manager: &Arc<Mutex<SourceManager>>,
    ) -> Result<()> {
        let target_path = self.lnreader_source_path(id);
        let mut transaction = self.install_transaction(id, &target_path)?;
        transaction.write(&target_path, contents)?;
        let staged_path = transaction.staged_path(&target_path)?;
        Source::write_meta_file(&staged_path, provenance, None)?;
        sync_staged_metadata(&transaction, &target_path)?;

        let staged_source = Source::from_lnreader_file(&staged_path, self, arc_manager)
            .context("staged LNReader plugin did not load")?;
        // Installing is an explicit user action with the network up, so the
        // probe runs right away: it writes the probe cache (later loads read
        // it and skip the JS evaluation) and the source is fully probed from
        // the start, showing its real manifest in the installed-sources list.
        staged_source
            .probe()
            .with_context(|| format!("failed to probe LNReader plugin {}", id.value()))?;
        validate_expected_sources(id, std::slice::from_ref(&staged_source))?;
        drop(staged_source);

        transaction.publish()?;
        let source = match Source::from_lnreader_file(&target_path, self, arc_manager)
            .and_then(|source| {
                source
                    .probe()
                    .with_context(|| format!("failed to probe LNReader plugin {}", id.value()))?;
                validate_expected_sources(id, std::slice::from_ref(&source))?;
                Ok(source)
            })
            .context("published LNReader plugin did not load")
        {
            Ok(source) => source,
            Err(error) => return Err(transaction.rollback_error(error)),
        };
        self.sources_by_id.insert(id.clone(), source);
        self.file_sources.insert(
            id.value().to_owned(),
            target_path.to_string_lossy().to_string(),
        );
        self.record_loaded_package(&target_path, vec![id.clone()]);
        transaction.commit();

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
        provenance: impl Into<crate::source_catalog::SourceProvenance>,
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
        let metadata_path = target_path.with_extension("json");
        let mut transaction = self.install_transaction(id, &target_path)?;
        transaction.write(&target_path, code)?;
        transaction.write(&metadata_path, metadata.to_string())?;
        let staged_path = transaction.staged_path(&target_path)?;
        Source::write_meta_file(&staged_path, provenance, None)?;
        sync_staged_metadata(&transaction, &target_path)?;

        let staged_source = Source::from_mangayomi_file(&staged_path, self, arc_manager)
            .context("staged MangaYomi extension did not load")?;
        // See `install_lnreader_source`: the probe runs eagerly so the probe
        // cache is written and the source is fully probed from the start.
        staged_source
            .probe()
            .with_context(|| format!("failed to probe MangaYomi extension {}", id.value()))?;
        validate_expected_sources(id, std::slice::from_ref(&staged_source))?;
        drop(staged_source);

        transaction.publish()?;
        let source = match Source::from_mangayomi_file(&target_path, self, arc_manager)
            .and_then(|source| {
                source.probe().with_context(|| {
                    format!("failed to probe MangaYomi extension {}", id.value())
                })?;
                validate_expected_sources(id, std::slice::from_ref(&source))?;
                Ok(source)
            })
            .context("published MangaYomi extension did not load")
        {
            Ok(source) => source,
            Err(error) => return Err(transaction.rollback_error(error)),
        };
        self.sources_by_id.insert(id.clone(), source);
        self.file_sources.insert(
            id.value().to_owned(),
            target_path.to_string_lossy().to_string(),
        );
        self.record_loaded_package(&target_path, vec![id.clone()]);
        transaction.commit();

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
        provenance: impl Into<crate::source_catalog::SourceProvenance>,
        arc_manager: &Arc<Mutex<SourceManager>>,
        languages: Option<&[String]>,
    ) -> Result<()> {
        if languages.is_some_and(|langs| langs.is_empty()) {
            bail!("keiyoushi language selection is empty");
        }

        let target_path = self.keiyoushi_source_path(id);
        let mut transaction = self.install_transaction(id, &target_path)?;
        transaction.write(&target_path, contents)?;
        let staged_path = transaction.staged_path(&target_path)?;
        Source::write_meta_file(&staged_path, provenance, languages.map(|l| l.to_vec()))?;
        sync_staged_metadata(&transaction, &target_path)?;

        let staged_sources = Source::from_keiyoushi_file(&staged_path, self, arc_manager)
            .context("staged Keiyoushi extension did not load")?;
        validate_expected_sources(id, &staged_sources)?;
        drop(staged_sources);

        transaction.publish()?;
        let sources = match Source::from_keiyoushi_file(&target_path, self, arc_manager)
            .and_then(|sources| {
                validate_expected_sources(id, &sources)?;
                Ok(sources)
            })
            .context("published Keiyoushi extension did not load")
        {
            Ok(sources) => sources,
            Err(error) => return Err(transaction.rollback_error(error)),
        };

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
        transaction.commit();

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

    /// Reads the exact catalog provenance stored beside an installed source.
    /// Older packages only contain the legacy `from` field and remain valid.
    pub fn source_provenance(
        &self,
        id: &SourceId,
    ) -> Option<crate::source_catalog::SourceProvenance> {
        let path = self.source_file_for_id(id)?;
        let meta_path = crate::source::BlockingSource::meta_source_path(&path).ok()?;
        let meta: crate::source::SourceMeta =
            serde_json::from_str(&fs::read_to_string(meta_path).ok()?).ok()?;
        Some(crate::source_catalog::SourceProvenance {
            source_of_source: meta.source_of_source,
            list_id: meta.catalog_list_id,
            provider_url: meta.provider_url,
            resolved_provider_url: meta.resolved_provider_url,
            version: meta.installed_version,
        })
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

fn source_package_artifact_paths(path: &Path) -> Result<Vec<PathBuf>> {
    let kind = source_package_kind(path).context("unsupported source package path")?;
    let mut paths = vec![
        path.to_path_buf(),
        crate::source::BlockingSource::meta_source_path(path)?,
    ];
    match kind {
        SourcePackageKind::Aidoku => {}
        SourcePackageKind::LnReader => paths.push(source_artifact_with_suffix(
            path,
            crate::source::lnreader::LNREADER_FILE_SUFFIX,
            crate::source::lnreader::LNREADER_PROBE_SUFFIX,
        )?),
        SourcePackageKind::MangaYomi => {
            paths.push(path.with_extension("json"));
            let package_suffix =
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.ends_with(crate::source::mangayomi::MANGA_YOMI_JS_FILE_SUFFIX)
                    })
                {
                    crate::source::mangayomi::MANGA_YOMI_JS_FILE_SUFFIX
                } else {
                    crate::source::mangayomi::MANGA_YOMI_FILE_SUFFIX
                };
            paths.push(source_artifact_with_suffix(
                path,
                package_suffix,
                crate::source::mangayomi::MANGA_YOMI_PROBE_SUFFIX,
            )?);
        }
        SourcePackageKind::Keiyoushi => paths.push(source_artifact_with_suffix(
            path,
            crate::source::keiyoushi::KEIYOUSHI_FILE_SUFFIX,
            crate::source::keiyoushi::KEIYOUSHI_PROBE_SUFFIX,
        )?),
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn source_artifact_with_suffix(
    package_path: &Path,
    package_suffix: &str,
    artifact_suffix: &str,
) -> Result<PathBuf> {
    let file_name = package_path
        .file_name()
        .and_then(|name| name.to_str())
        .context("source package file name is not valid UTF-8")?;
    let stem = file_name
        .strip_suffix(package_suffix)
        .context("source package has an unexpected suffix")?;
    Ok(package_path.with_file_name(format!("{stem}{artifact_suffix}")))
}

fn sync_staged_metadata(
    transaction: &SourceInstallTransaction,
    target_package: &Path,
) -> Result<()> {
    let staged_package = transaction.staged_path(target_package)?;
    let staged_meta = crate::source::BlockingSource::meta_source_path(&staged_package)?;
    fs::File::open(&staged_meta)
        .and_then(|file| file.sync_all())
        .with_context(|| {
            format!(
                "couldn't sync source artifact '{}'",
                source_file_label(&staged_meta)
            )
        })
}

fn validate_expected_sources(expected: &SourceId, sources: &[Source]) -> Result<()> {
    if sources.is_empty() {
        bail!("source package did not provide any sources")
    }
    let mut ids = BTreeSet::new();
    for source in sources {
        let id = SourceId::new(source.manifest().info.id.clone());
        if !ids.insert(id.value().clone()) {
            bail!("source package contains duplicate source ids")
        }
    }
    if !ids.contains(expected.value()) {
        bail!(
            "source package did not provide the requested source '{}'",
            expected.value()
        )
    }
    Ok(())
}

#[cfg(unix)]
fn sync_source_directory(path: &Path) {
    if let Err(error) = fs::File::open(path).and_then(|directory| directory.sync_all()) {
        log::warn!("couldn't sync source directory {}: {error}", path.display());
    }
}

#[cfg(not(unix))]
fn sync_source_directory(_path: &Path) {}

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
    use std::io::{Cursor, Write};

    use tempfile::tempdir;
    use zip::{write::FileOptions, ZipWriter};

    use super::*;

    fn test_aix_bytes(source_id: &str, name: &str) -> Vec<u8> {
        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
        let options: FileOptions<'_, ()> = FileOptions::default();
        archive
            .start_file("Payload/source.json", options)
            .expect("start source manifest");
        write!(
            archive,
            r#"{{"info":{{"id":"{source_id}","name":"{name}","version":1}}}}"#
        )
        .expect("write source manifest");
        archive
            .finish()
            .expect("finish test source archive")
            .into_inner()
    }

    fn write_test_aix(path: &Path, source_id: &str, name: &str) {
        fs::write(path, test_aix_bytes(source_id, name)).expect("write test source archive");
    }

    fn staging_directories(path: &Path) -> Vec<PathBuf> {
        fs::read_dir(path)
            .expect("read sources directory")
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".rakuyomi-install-"))
            })
            .collect()
    }

    #[test]
    fn transaction_rollback_restores_package_and_sidecar() {
        let directory = tempdir().expect("create sources directory");
        let package = directory.path().join("fixture.aix");
        let sidecar = crate::source::BlockingSource::meta_source_path(&package)
            .expect("derive metadata path");
        fs::write(&package, b"old package").expect("write old package");
        fs::write(&sidecar, b"old sidecar").expect("write old sidecar");

        let mut transaction = SourceInstallTransaction::new(
            directory.path(),
            source_package_artifact_paths(&package).expect("list artifacts"),
        )
        .expect("create transaction");
        transaction
            .write(&package, b"new package")
            .expect("stage package");
        transaction
            .write(&sidecar, b"new sidecar")
            .expect("stage sidecar");
        transaction.publish().expect("publish replacement");
        assert_eq!(
            fs::read(&package).expect("read new package"),
            b"new package"
        );

        transaction.rollback().expect("restore prior artifacts");

        assert_eq!(fs::read(package).expect("read old package"), b"old package");
        assert_eq!(fs::read(sidecar).expect("read old sidecar"), b"old sidecar");
    }

    #[test]
    fn failed_aidoku_replacement_keeps_prior_package_and_loaded_source() {
        let directory = tempdir().expect("create sources directory");
        let manager = Arc::new(Mutex::new(
            SourceManager::from_folder(directory.path().to_path_buf(), Settings::default())
                .expect("create source manager"),
        ));
        let id = SourceId::new("fixture.source".to_owned());
        let old = test_aix_bytes(id.value(), "Old Source");
        let mut guard = manager.blocking_lock();
        guard
            .install_source(&id, &old, "old-provider".to_string(), &manager)
            .expect("install prior package");
        let package = guard.source_path(&id);
        let sidecar = crate::source::BlockingSource::meta_source_path(&package)
            .expect("derive metadata path");
        let old_sidecar = fs::read(&sidecar).expect("read prior sidecar");

        let result = guard.install_source(
            &id,
            b"not a source archive",
            "new-provider".to_string(),
            &manager,
        );

        assert!(result.is_err());
        assert_eq!(fs::read(package).expect("read retained package"), old);
        assert_eq!(
            fs::read(sidecar).expect("read retained sidecar"),
            old_sidecar
        );
        assert_eq!(
            guard
                .sources_by_id
                .get(&id)
                .expect("prior source remains loaded")
                .manifest()
                .info
                .name,
            "Old Source"
        );
        assert!(staging_directories(directory.path()).is_empty());
    }

    #[test]
    fn wrong_aidoku_identity_cannot_replace_prior_package() {
        let directory = tempdir().expect("create sources directory");
        let manager = Arc::new(Mutex::new(
            SourceManager::from_folder(directory.path().to_path_buf(), Settings::default())
                .expect("create source manager"),
        ));
        let id = SourceId::new("fixture.source".to_owned());
        let old = test_aix_bytes(id.value(), "Old Source");
        let wrong = test_aix_bytes("different.source", "Wrong Source");
        let mut guard = manager.blocking_lock();
        guard
            .install_source(&id, &old, "old-provider".to_string(), &manager)
            .expect("install prior package");

        let result = guard.install_source(&id, wrong, "new-provider".to_string(), &manager);

        assert!(result.is_err());
        assert_eq!(
            guard
                .sources_by_id
                .get(&id)
                .expect("prior source remains loaded")
                .manifest()
                .info
                .name,
            "Old Source"
        );
        assert_eq!(fs::read(guard.source_path(&id)).expect("read package"), old);
        assert!(staging_directories(directory.path()).is_empty());
    }

    #[test]
    fn successful_aidoku_replacement_commits_package_and_cleans_rollback() {
        let directory = tempdir().expect("create sources directory");
        let manager = Arc::new(Mutex::new(
            SourceManager::from_folder(directory.path().to_path_buf(), Settings::default())
                .expect("create source manager"),
        ));
        let id = SourceId::new("fixture.source".to_owned());
        let old = test_aix_bytes(id.value(), "Old Source");
        let new = test_aix_bytes(id.value(), "New Source");
        let mut guard = manager.blocking_lock();
        guard
            .install_source(&id, old, "old-provider".to_string(), &manager)
            .expect("install prior package");

        guard
            .install_source(&id, &new, "new-provider".to_string(), &manager)
            .expect("install replacement");

        assert_eq!(fs::read(guard.source_path(&id)).expect("read package"), new);
        assert_eq!(
            guard
                .sources_by_id
                .get(&id)
                .expect("replacement source is loaded")
                .manifest()
                .info
                .name,
            "New Source"
        );
        assert_eq!(
            guard
                .source_provenance(&id)
                .and_then(|provenance| provenance.source_of_source),
            Some("new-provider".to_owned())
        );
        assert!(staging_directories(directory.path()).is_empty());
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
