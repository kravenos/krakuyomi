use anyhow::Result;

use crate::{database::Database, model::Manga, source_collection::SourceCollection};

pub async fn get_manga_library(
    db: &Database,
    source_collection: &impl SourceCollection,
    library_sorting_mode: &crate::settings::LibrarySortingMode,
) -> Result<Vec<Manga>> {
    let mut mangas = db
        .get_manga_library_with_read_count(source_collection, library_sorting_mode)
        .await?;

    fill_cached_chapter_counts(db, &mut mangas).await?;

    Ok(mangas)
}

/// Fills cached total and explicit read counts from one grouped query, without
/// per-manga queries or filesystem access.
pub(crate) async fn fill_cached_chapter_counts(db: &Database, mangas: &mut [Manga]) -> Result<()> {
    if mangas.is_empty() {
        return Ok(());
    }

    let counts = db.get_cached_chapter_counts().await?;

    for manga in mangas.iter_mut() {
        let key = (
            manga.information.id.source_id().value().clone(),
            manga.information.id.value().clone(),
        );
        if let Some(counts) = counts.get(&key) {
            manga.total_chapters_count = Some(counts.total);
            manga.read_chapters_count = Some(counts.read);
        }
    }

    Ok(())
}
