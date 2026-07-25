use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use log::{info, warn};
use shared::settings::{backup_path_for, Settings};

enum SettingsRead {
    Missing,
    Invalid(anyhow::Error),
    Valid(Settings),
}

/// Loads settings, restoring a last-known-good backup after JSON corruption.
/// Missing files are initialized from `default_settings_json`; non-recoverable
/// I/O failures are returned without moving or replacing the original path.
pub(crate) fn load_settings_or_recover(
    settings_path: &Path,
    default_settings_json: &str,
) -> Result<(Settings, Option<String>)> {
    let (primary_was_missing, primary_error) = match read_settings(settings_path)? {
        SettingsRead::Valid(settings) => return Ok((settings, None)),
        SettingsRead::Missing => (true, "file does not exist".to_owned()),
        SettingsRead::Invalid(error) => (false, format!("{error:#}")),
    };

    warn!(
        "couldn't read settings file at {}: {primary_error}; attempting recovery",
        settings_path.display()
    );

    let backup_path = backup_path_for(settings_path);
    match read_settings(&backup_path)? {
        SettingsRead::Valid(settings) => {
            let invalid_path = preserve_invalid_settings_file(settings_path)?;
            settings
                .save_to_file(settings_path)
                .with_context(|| "couldn't restore settings from backup")?;

            let preserved_note = invalid_path
                .map(|path| format!(" The unreadable file was kept at {}.", path.display()))
                .unwrap_or_default();
            Ok((
                settings,
                Some(format!(
                    "Rakuyomi recovered your settings from the last-known-good backup after the primary settings file couldn't be read ({primary_error}).{preserved_note}"
                )),
            ))
        }
        backup_result => {
            let (backup_was_missing, backup_error) = match backup_result {
                SettingsRead::Missing => (true, "file does not exist".to_owned()),
                SettingsRead::Invalid(error) => (false, format!("{error:#}")),
                SettingsRead::Valid(_) => unreachable!(),
            };
            if primary_was_missing && backup_was_missing {
                info!(
                    "settings file not found at {}, creating default",
                    settings_path.display()
                );
                let settings = Settings::from_json(default_settings_json)
                    .with_context(|| "couldn't parse default settings")?;
                settings
                    .save_to_file(settings_path)
                    .with_context(|| "couldn't write default settings")?;
                return Ok((settings, None));
            }

            let invalid_primary = preserve_invalid_settings_file(settings_path)?;
            let invalid_backup = preserve_invalid_settings_file(&backup_path)?;
            let settings = Settings::from_json(default_settings_json)
                .with_context(|| "couldn't parse default settings")?;
            settings
                .save_to_file(settings_path)
                .with_context(|| "couldn't write default settings")?;

            let preserved_paths = [invalid_primary, invalid_backup]
                .into_iter()
                .flatten()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>();
            let preserved_note = if preserved_paths.is_empty() {
                String::new()
            } else {
                format!(
                    " Unreadable files were preserved at {}.",
                    preserved_paths.join(" and ")
                )
            };

            Ok((
                settings,
                Some(format!(
                    "Rakuyomi couldn't read the primary settings file ({primary_error}) or its backup ({backup_error}), so it started with default settings.{preserved_note}"
                )),
            ))
        }
    }
}

fn read_settings(path: &Path) -> Result<SettingsRead> {
    let contents = match fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SettingsRead::Missing)
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("couldn't access settings file at {}", path.display()))
        }
    };

    let contents = match std::str::from_utf8(&contents) {
        Ok(contents) => contents,
        Err(error) => return Ok(SettingsRead::Invalid(error.into())),
    };

    Ok(match Settings::from_json(contents) {
        Ok(settings) => SettingsRead::Valid(settings),
        Err(error) => SettingsRead::Invalid(error),
    })
}

fn preserve_invalid_settings_file(path: &Path) -> Result<Option<PathBuf>> {
    match fs::metadata(path) {
        Ok(metadata) if !metadata.is_file() => {
            anyhow::bail!("settings path is not a regular file: {}", path.display())
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("couldn't inspect settings file at {}", path.display()))
        }
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let invalid_path =
        path.with_extension(format!("invalid-{timestamp}-{}.json", std::process::id()));
    fs::rename(path, &invalid_path).with_context(|| {
        format!(
            "couldn't preserve unreadable settings file {} at {}",
            path.display(),
            invalid_path.display()
        )
    })?;

    Ok(Some(invalid_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_SETTINGS_JSON: &str = r#"{"languages":["en"]}"#;

    fn temp_dir(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "rakuyomi-settings-recovery-{tag}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temporary test directory");
        dir
    }

    #[test]
    fn corrupt_primary_recovers_the_last_known_good_backup() {
        let dir = temp_dir("backup");
        let settings_path = dir.join("settings.json");
        let backup_path = backup_path_for(&settings_path);
        let mut expected = Settings::from_json(DEFAULT_SETTINGS_JSON).unwrap();
        expected.languages = vec!["nl".to_owned(), "en".to_owned()];

        fs::write(
            &backup_path,
            serde_json::to_vec_pretty(&expected).expect("serialize backup settings"),
        )
        .expect("write last-known-good backup");
        fs::write(&settings_path, "{\n  \"languages\": [\"nl\"]")
            .expect("write truncated primary settings");

        let (recovered, message) = load_settings_or_recover(&settings_path, DEFAULT_SETTINGS_JSON)
            .expect("recover settings from backup");

        assert_eq!(recovered.languages, expected.languages);
        assert!(message
            .as_deref()
            .is_some_and(|message| message.contains("backup")));
        assert_eq!(
            Settings::from_file(&settings_path)
                .expect("restored primary should parse")
                .languages,
            expected.languages
        );

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn non_file_primary_is_not_misclassified_as_corrupt_json() {
        let dir = temp_dir("non-file");
        let settings_path = dir.join("settings.json");
        fs::create_dir(&settings_path).expect("create directory at settings path");

        let result = load_settings_or_recover(&settings_path, DEFAULT_SETTINGS_JSON);
        let remained_a_directory = settings_path.is_dir();

        fs::remove_dir_all(dir).ok();

        assert!(result.is_err());
        assert!(remained_a_directory);
    }

    #[test]
    fn corrupt_legacy_primary_is_preserved_before_defaults_are_created() {
        let dir = temp_dir("legacy");
        let settings_path = dir.join("settings.json");
        let truncated = b"{\n  \"languages\": [\"nl\"]";
        fs::write(&settings_path, truncated).expect("write truncated legacy settings");

        let (settings, message) = load_settings_or_recover(&settings_path, DEFAULT_SETTINGS_JSON)
            .expect("recover legacy settings with defaults");

        assert_eq!(settings.languages, vec!["en"]);
        assert!(message
            .as_deref()
            .is_some_and(|message| message.contains("default settings")));
        Settings::from_file(&settings_path).expect("replacement settings should parse");

        let preserved = fs::read_dir(&dir)
            .expect("list settings directory")
            .filter_map(|entry| entry.ok())
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("settings.invalid-")
            })
            .expect("truncated settings should be preserved");
        assert_eq!(fs::read(preserved.path()).unwrap(), truncated);

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn missing_primary_and_backup_create_defaults_without_a_recovery_warning() {
        let dir = temp_dir("first-run");
        let settings_path = dir.join("settings.json");

        let (settings, message) = load_settings_or_recover(&settings_path, DEFAULT_SETTINGS_JSON)
            .expect("create default settings");

        assert!(message.is_none());
        assert_eq!(settings.languages, vec!["en"]);
        Settings::from_file(&settings_path).expect("primary settings should parse");
        Settings::from_file(&backup_path_for(&settings_path))
            .expect("backup settings should parse");

        fs::remove_dir_all(dir).ok();
    }
}
