//! Translate saved MangaFire references only at the source boundary.
//!
//! Installed next-SDK v8 expects short keys, while older library entries retain
//! `/title/<hid>-<slug>` and `chapter/<number>` keys. Keep those stored identities
//! intact: progress and downloaded-file lookup depend on them.

use std::collections::HashSet;

use anyhow::{bail, Result};

use super::SourceManifest;

/// Returns a source key only for the verified package and saved-key format.
pub(super) fn source_manga_key<'a>(
    manifest: &SourceManifest,
    next_sdk: bool,
    saved_key: &'a str,
) -> Option<&'a str> {
    if !next_sdk
        || manifest.info.id != "multi.mangafire"
        || manifest.info.version.as_u64() != Some(8)
    {
        return None;
    }
    let (key, slug) = saved_key.strip_prefix("/title/")?.split_once('-')?;
    if key.is_empty()
        || !key.bytes().all(|byte| byte.is_ascii_alphanumeric())
        || slug.is_empty()
        || !slug
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return None;
    }
    Some(key)
}

/// Strips only a recognized saved chapter prefix; opaque keys pass through.
pub(super) fn source_chapter_key(saved_key: &str) -> Option<&str> {
    saved_key
        .strip_prefix("chapter/")
        .filter(|key| numeric(key))
}

/// Restores response identities before callers can update cached library data.
pub(super) fn restore_saved_keys(manga: &mut aidoku::Manga, saved_key: String) -> Result<()> {
    manga.key = saved_key;
    if let Some(chapters) = &mut manga.chapters {
        for chapter in chapters.iter_mut() {
            if numeric(&chapter.key) {
                chapter.key.insert_str(0, "chapter/");
            }
        }
        let mut keys = HashSet::with_capacity(chapters.len());
        for chapter in chapters.iter() {
            if !keys.insert(chapter.key.as_str()) {
                bail!("MangaFire returned conflicting chapter references; refresh was not applied");
            }
        }
    }
    Ok(())
}

fn numeric(key: &str) -> bool {
    !key.is_empty() && key.bytes().all(|byte| byte.is_ascii_digit())
}
