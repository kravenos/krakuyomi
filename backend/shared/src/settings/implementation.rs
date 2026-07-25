use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

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

    /// Writes the settings to `path` atomically: the serialized contents are
    /// written to a temporary file in the same directory, flushed and synced,
    /// and only then renamed over the target.
    ///
    /// Writing in place would truncate the existing file up front, so an
    /// interrupted write (power loss, or the server being killed while it
    /// saves) would leave a half-written file that fails to parse and
    /// prevents the plugin from starting. A rename within the same
    /// filesystem is atomic, so readers only ever observe the old file or
    /// the complete new one.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let tmp_path = temporary_path_for(path);

        {
            let file = File::create(&tmp_path).with_context(|| {
                format!("couldn't create temporary file at {}", tmp_path.display())
            })?;
            let mut writer = BufWriter::new(file);

            serde_json_lenient::to_writer_pretty(&mut writer, self)
                .with_context(|| "couldn't serialize settings")?;

            writer.flush().with_context(|| "couldn't flush settings")?;
            // Make sure the contents actually reached storage before the
            // rename publishes them.
            writer
                .get_ref()
                .sync_all()
                .with_context(|| "couldn't sync settings to disk")?;
        }

        fs::rename(&tmp_path, path).with_context(|| {
            format!(
                "couldn't move {} into place at {}",
                tmp_path.display(),
                path.display()
            )
        })?;

        Ok(())
    }
}

/// Returns a temporary path next to `path`, so the final rename stays within
/// the same filesystem.
fn temporary_path_for(path: &Path) -> PathBuf {
    let mut file_name = path.file_name().unwrap_or_default().to_os_string();
    file_name.push(".tmp");

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
        // The temporary file must not be left behind.
        assert!(!temporary_path_for(&path).exists());

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
        assert_eq!(tmp.file_name().unwrap(), "settings.json.tmp");
    }
}
