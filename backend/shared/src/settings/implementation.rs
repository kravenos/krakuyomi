use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};

use super::schema::Settings;

impl Settings {
    /// Parses settings from JSON and applies runtime-dependent defaults.
    pub fn from_json(contents: &str) -> Result<Self> {
        let settings = serde_json_lenient::from_str(contents)
            .with_context(|| "Couldn't parse settings contents")?;

        Ok(apply_runtime_defaults(settings))
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| "Couldn't open file")?;
        let settings: Settings = serde_json_lenient::from_reader(file)
            .with_context(|| "Couldn't parse file contents")?;

        Ok(apply_runtime_defaults(settings))
    }

    /// Writes the settings and a last-known-good backup atomically. Each file
    /// is written to a unique temporary file in the same directory, flushed
    /// and synced, and only then renamed over its target.
    ///
    /// Writing in place would truncate the existing file up front, so an
    /// interrupted write (power loss, or the server being killed while it
    /// saves) would leave a half-written file that fails to parse and
    /// prevents the plugin from starting. A rename within the same
    /// filesystem is atomic, so readers only ever observe the old file or
    /// the complete new one.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let contents = serde_json_lenient::to_vec_pretty(self)
            .with_context(|| "couldn't serialize settings")?;

        // Publish the backup first. If the process stops between these two
        // writes, the primary still contains the previous valid settings and
        // the backup contains the requested update; either file is usable.
        atomic_write(&backup_path_for(path), &contents)?;
        atomic_write(path, &contents)?;

        Ok(())
    }
}

fn apply_runtime_defaults(mut settings: Settings) -> Settings {
    if settings.concurrent_requests_pages.is_none() {
        settings.concurrent_requests_pages =
            Some(if cfg!(target_arch = "arm") && cfg!(target_os = "linux") {
                4
            } else {
                5
            });
    }

    settings
}

/// Returns the last-known-good backup path for `path`.
pub fn backup_path_for(path: &Path) -> PathBuf {
    path.with_extension("backup.json")
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let tmp_path = temporary_path_for(path);
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp_path)
            .with_context(|| format!("couldn't create temporary file at {}", tmp_path.display()))?;

        file.write_all(contents)
            .with_context(|| "couldn't write settings")?;
        file.flush().with_context(|| "couldn't flush settings")?;
        file.sync_all()
            .with_context(|| "couldn't sync settings to disk")?;
        drop(file);

        fs::rename(&tmp_path, path).with_context(|| {
            format!(
                "couldn't move {} into place at {}",
                tmp_path.display(),
                path.display()
            )
        })?;

        sync_parent_directory(path);
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }

    result
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    if let Err(error) = File::open(parent).and_then(|directory| directory.sync_all()) {
        log::warn!(
            "couldn't sync settings directory {}: {error}",
            parent.display()
        );
    }
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) {}

/// Returns a temporary path next to `path`, so the final rename stays within
/// the same filesystem.
fn temporary_path_for(path: &Path) -> PathBuf {
    static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    let mut file_name = path.file_name().unwrap_or_default().to_os_string();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    file_name.push(format!(
        ".tmp-{}-{timestamp}-{sequence}",
        std::process::id()
    ));

    match path.parent() {
        Some(parent) => parent.join(file_name),
        None => PathBuf::from(file_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Settings as an empty settings file would produce them, i.e. with every
    /// serde default applied. `Settings::default()` is not equivalent: it
    /// leaves `storage_size_limit` at zero bytes, which does not round-trip
    /// through the size (de)serializer.
    fn default_settings() -> Settings {
        serde_json_lenient::from_str("{}").expect("empty settings object should parse")
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rakuyomi-settings-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_to_file_writes_parseable_settings() {
        let dir = temp_dir("write");
        let path = dir.join("settings.json");

        default_settings().save_to_file(&path).unwrap();

        Settings::from_file(&path).expect("settings written should parse back");
        // No temporary files may be left behind.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_to_file_keeps_a_parseable_last_known_good_backup() {
        let dir = temp_dir("backup");
        let path = dir.join("settings.json");
        let backup_path = path.with_extension("backup.json");
        let mut expected = default_settings();
        expected.languages = vec!["nl".to_owned(), "en".to_owned()];

        expected.save_to_file(&path).unwrap();

        let backup = Settings::from_file(&backup_path).expect("backup should parse");
        assert_eq!(backup.languages, expected.languages);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_to_file_replaces_a_truncated_file() {
        let dir = temp_dir("replace");
        let path = dir.join("settings.json");

        // A truncated file, like one left behind by an interrupted write.
        fs::write(&path, "{\n  \"storage_path\": \"/some/pa").unwrap();

        default_settings().save_to_file(&path).unwrap();

        Settings::from_file(&path).expect("settings should parse after overwriting a corrupt file");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn temporary_path_is_a_sibling_of_the_target() {
        let tmp = temporary_path_for(Path::new("/home/user/rakuyomi/settings.json"));

        assert_eq!(tmp.parent(), Some(Path::new("/home/user/rakuyomi")));
        assert!(tmp
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("settings.json.tmp-"));
    }
}
