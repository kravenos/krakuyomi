use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use log::{info, warn};
use shared::settings::{backup_path_for, Settings};

enum SettingsRead {
    Missing,
    Invalid(anyhow::Error),
    Valid(Box<Settings>),
}

/// Loads settings and recovers from the last-known-good backup when the
/// primary JSON is unreadable. The returned message is safe to show to a user.
pub(crate) fn load_settings_or_recover(
    settings_path: &Path,
    default_settings_json: &str,
) -> Result<(Settings, Option<String>)> {
    let (primary_was_missing, primary_error) = match read_settings(settings_path)? {
        SettingsRead::Valid(settings) => return Ok((*settings, None)),
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
            let preserved_primary = preserve_invalid_settings_file(settings_path)?.is_some();
            settings
                .save_to_file(settings_path)
                .with_context(|| "couldn't restore settings from backup")?;

            let evidence_note = if preserved_primary {
                " The unreadable file was preserved beside the settings file."
            } else {
                ""
            };
            Ok((
                *settings,
                Some(format!(
                    "Rakuyomi recovered your settings from its last-known-good backup.{evidence_note}"
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
                let settings = parse_default_settings(default_settings_json)?;
                settings
                    .save_to_file(settings_path)
                    .with_context(|| "couldn't write default settings")?;
                return Ok((settings, None));
            }

            preserve_invalid_settings_file(settings_path)?;
            preserve_invalid_settings_file(&backup_path)?;

            warn!(
                "settings backup at {} could not be read: {backup_error}; using defaults",
                backup_path.display()
            );
            let settings = parse_default_settings(default_settings_json)?;
            settings
                .save_to_file(settings_path)
                .with_context(|| "couldn't write default settings")?;

            Ok((
                settings,
                Some(
                    "Rakuyomi could not read a usable settings copy, so it started with defaults. Any unreadable files were preserved beside the settings file."
                        .to_owned(),
                ),
            ))
        }
    }
}

fn parse_default_settings(default_settings_json: &str) -> Result<Settings> {
    Settings::from_json(default_settings_json)
        .with_context(|| "couldn't parse bundled default settings")
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
        Ok(settings) => SettingsRead::Valid(Box::new(settings)),
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
    use tempfile::tempdir;

    use super::*;

    const DEFAULT_SETTINGS_JSON: &str = r#"{"languages":["en"]}"#;

    #[test]
    fn corrupt_primary_recovers_the_backup_and_preserves_evidence() {
        let directory = tempdir().expect("create temporary settings directory");
        let settings_path = directory.path().join("settings.json");
        let backup_path = backup_path_for(&settings_path);
        let mut expected = Settings::from_json(DEFAULT_SETTINGS_JSON).expect("parse test settings");
        expected.languages = vec!["nl".to_owned(), "en".to_owned()];
        expected
            .save_to_file(&settings_path)
            .expect("write primary and backup");
        let corrupt = b"{\"languages\":[\"nl\"]";
        fs::write(&settings_path, corrupt).expect("write corrupt primary");

        let (recovered, message) = load_settings_or_recover(&settings_path, DEFAULT_SETTINGS_JSON)
            .expect("recover settings");

        assert_eq!(recovered.languages, expected.languages);
        assert!(message
            .as_deref()
            .is_some_and(|text| text.contains("backup")));
        assert!(!message
            .as_deref()
            .is_some_and(|text| text.contains(&directory.path().display().to_string())));
        assert_eq!(
            Settings::from_file(&settings_path)
                .expect("read restored primary")
                .languages,
            expected.languages
        );
        let preserved = fs::read_dir(directory.path())
            .expect("list settings directory")
            .filter_map(|entry| entry.ok())
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("settings.invalid-")
            })
            .expect("preserve corrupt primary");
        assert_eq!(fs::read(preserved.path()).expect("read evidence"), corrupt);
    }

    #[test]
    fn corrupt_primary_and_backup_use_defaults_without_deleting_evidence() {
        let directory = tempdir().expect("create temporary settings directory");
        let settings_path = directory.path().join("settings.json");
        let backup_path = backup_path_for(&settings_path);
        fs::write(&settings_path, b"{").expect("write corrupt primary");
        fs::write(&backup_path, b"[").expect("write corrupt backup");

        let (settings, message) = load_settings_or_recover(&settings_path, DEFAULT_SETTINGS_JSON)
            .expect("recover with defaults");

        assert_eq!(settings.languages, vec!["en"]);
        assert!(message
            .as_deref()
            .is_some_and(|text| text.contains("defaults")));
        let preserved_count = fs::read_dir(directory.path())
            .expect("list settings directory")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".invalid-"))
            .count();
        assert_eq!(preserved_count, 2);
    }

    #[test]
    fn first_run_creates_primary_and_backup_without_warning() {
        let directory = tempdir().expect("create temporary settings directory");
        let settings_path = directory.path().join("settings.json");

        let (settings, message) = load_settings_or_recover(&settings_path, DEFAULT_SETTINGS_JSON)
            .expect("create default settings");

        assert_eq!(settings.languages, vec!["en"]);
        assert!(message.is_none());
        Settings::from_file(&settings_path).expect("read primary");
        Settings::from_file(&backup_path_for(&settings_path)).expect("read backup");
    }

    #[test]
    fn non_file_primary_is_not_moved_or_replaced() {
        let directory = tempdir().expect("create temporary settings directory");
        let settings_path = directory.path().join("settings.json");
        fs::create_dir(&settings_path).expect("create directory at settings path");

        let result = load_settings_or_recover(&settings_path, DEFAULT_SETTINGS_JSON);

        assert!(result.is_err());
        assert!(settings_path.is_dir());
    }
}
