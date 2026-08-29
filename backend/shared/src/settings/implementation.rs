use std::{fs::File, io::Write, path::Path};

use anyhow::{Context, Result};
use tempfile::NamedTempFile;

use super::schema::Settings;

impl Settings {
    pub fn from_file(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| "Couldn't open file")?;
        let mut settings: Settings = serde_json_lenient::from_reader(file)
            .with_context(|| "Couldn't parse file contents")?;

        if settings.concurrent_requests_pages.is_none() {
            settings.concurrent_requests_pages =
                Some(if cfg!(target_arch = "arm") && cfg!(target_os = "linux") {
                    4
                } else {
                    5
                });
        }

        Ok(settings)
    }

    /// Atomically publishes a complete settings file at `path`.
    ///
    /// The temporary file is created beside the target so the final replace
    /// stays on one filesystem. A failed write or sync leaves the previous
    /// settings file untouched.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = NamedTempFile::new_in(parent).with_context(|| {
            format!(
                "couldn't create temporary settings file in {}",
                parent.display()
            )
        })?;

        serde_json_lenient::to_writer_pretty(temporary.as_file_mut(), self)
            .with_context(|| "couldn't serialize settings")?;
        temporary
            .as_file_mut()
            .write_all(b"\n")
            .with_context(|| "couldn't finish settings write")?;
        temporary
            .as_file_mut()
            .flush()
            .with_context(|| "couldn't flush settings")?;
        temporary
            .as_file()
            .sync_all()
            .with_context(|| "couldn't sync settings to disk")?;

        temporary
            .persist(path)
            .map_err(|error| error.error)
            .with_context(|| format!("couldn't publish settings at {}", path.display()))?;

        sync_parent_directory(parent);

        Ok(())
    }
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) {
    if let Err(error) = File::open(parent).and_then(|directory| directory.sync_all()) {
        log::warn!(
            "couldn't sync settings directory {}: {error}",
            parent.display()
        );
    }
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn default_settings() -> Settings {
        serde_json_lenient::from_str("{}").expect("empty settings should use schema defaults")
    }

    #[test]
    fn successful_save_publishes_one_complete_file() {
        let directory = tempdir().expect("create temporary settings directory");
        let path = directory.path().join("settings.json");
        let mut expected = default_settings();
        expected.languages = vec!["nl".to_owned(), "en".to_owned()];

        expected.save_to_file(&path).expect("save settings");

        let actual = Settings::from_file(&path).expect("read saved settings");
        assert_eq!(actual.languages, expected.languages);
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("list settings directory")
                .count(),
            1,
            "a successful save must not leave a temporary file"
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_save_keeps_the_previous_file_intact() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempdir().expect("create temporary settings directory");
        let path = directory.path().join("settings.json");
        let original = b"{\"languages\":[\"en\"]}\n";
        fs::write(&path, original).expect("write original settings");

        let original_mode = fs::metadata(directory.path())
            .expect("read directory metadata")
            .permissions()
            .mode();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o555))
            .expect("make directory read-only");

        let result = default_settings().save_to_file(&path);

        fs::set_permissions(directory.path(), fs::Permissions::from_mode(original_mode))
            .expect("restore directory permissions");

        assert!(result.is_err(), "publishing a new file should fail");
        assert_eq!(
            fs::read(&path).expect("read original settings after failure"),
            original,
            "a failed save must not truncate or replace the previous settings"
        );
    }
}
