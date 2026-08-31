use crate::{
    chapter_storage::ChapterStorage,
    database::Database,
    model::{Manga, MangaId, MangaInformation, MangaState, SourceInformation},
    settings::{SearchViewMode, Settings},
    source_collection::SourceCollection,
    source_health::{
        SourceErrorCategory, SourceHealthObservation, SourceHealthStore, SourceOperationClass,
        SourceOperationError,
    },
};
use futures::{stream, StreamExt};
use log::warn;
use std::collections::HashSet;
use tokio::time::timeout;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;
use unicode_normalization::UnicodeNormalization;

const CONCURRENT_SEARCH_REQUESTS: usize = 5;
const CONCURRENT_POSTER_DOWNLOADS: usize = 4;
const POSTER_DOWNLOAD_TIMEOUT_SECS: u64 = 10;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct SearchError {
    pub source_id: String,
    pub category: SourceErrorCategory,
    pub message: String,
}

/// One source's explicit outcome for a search page, including zero results.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct SourceSearchOutcome {
    pub source_id: String,
    pub source_name: String,
    pub result_count: usize,
    pub has_next_page: bool,
    pub error: Option<SearchError>,
}

#[allow(clippy::too_many_arguments)]
pub async fn search_mangas(
    source_collection: &impl SourceCollection,
    db: &Database,
    chapter_storage: &ChapterStorage,
    settings: &Settings,
    cancellation_token: CancellationToken,
    query: String,
    included_source_ids: &Option<HashSet<String>>,
    page: i32,
    seconds: u64,
    source_health: &SourceHealthStore,
) -> Result<(Vec<Manga>, Vec<SourceSearchOutcome>, bool), Error> {
    // FIXME this looks awful
    let query = &query;

    // FIXME this kinda of works because cloning a source is cheap
    // (it has internal mutability yadda yadda).
    // we can't keep `source_collection` alive across async await points
    // because lifetimes fuckery
    let sources = source_collection
        .sources()
        .into_iter()
        .filter(|source| source_is_included(included_source_ids, &source.manifest().info.id))
        .cloned()
        .collect::<Vec<_>>();

    let source_results: Vec<(
        SourceMangaSearchResults,
        Option<SearchError>,
        bool,
        Option<SourceHealthObservation>,
    )> = stream::iter(sources)
        .map(|source| {
            let cancellation_token = cancellation_token.clone();
            let query = query.to_string();
            let chapter_storage = chapter_storage.clone();

            async move {
                let token = cancellation_token.child_token();
                let item_key = query.clone();

                let fetch_task = async { source.search_mangas(token.clone(), query, page).await };

                let source_id = source.manifest().info.id.clone();
                let (manga_informations, has_next, error, observation) =
                    match timeout(Duration::from_secs(seconds), fetch_task).await {
                        Ok(Ok((source_mangas, has_next_page))) => (
                            source_mangas
                                .into_iter()
                                .map(MangaInformation::from)
                                .collect(),
                            has_next_page,
                            None,
                            Some(SourceHealthObservation::success(
                                source_id.clone(),
                                SourceOperationClass::Search,
                                &item_key,
                            )),
                        ),

                        Ok(Err(e)) => {
                            let error = SourceOperationError::classify(e);
                            warn!(
                                "failed to search mangas from source {}: {:#}",
                                source_id,
                                error.cause()
                            );

                            (
                                vec![],
                                false,
                                Some(SearchError {
                                    source_id: source_id.clone(),
                                    category: error.category(),
                                    message: error.safe_message().to_owned(),
                                }),
                                Some(SourceHealthObservation::failure(
                                    source_id.clone(),
                                    SourceOperationClass::Search,
                                    &item_key,
                                    &error,
                                )),
                            )
                        }

                        Err(_) => {
                            token.cancel();
                            let error = SourceOperationError::timeout();

                            (
                                vec![],
                                false,
                                Some(SearchError {
                                    source_id: source_id.clone(),
                                    category: error.category(),
                                    message: error.safe_message().to_owned(),
                                }),
                                Some(SourceHealthObservation::failure(
                                    source_id.clone(),
                                    SourceOperationClass::Search,
                                    &item_key,
                                    &error,
                                )),
                            )
                        }
                    };

                // Write through to the database
                let _ = db
                    .upsert_cached_manga_information(&manga_informations)
                    .await;

                if settings.search_view_mode != SearchViewMode::Base {
                    // Download posters concurrently so cover/grid view can render them
                    let poster_items: Vec<(MangaId, url::Url)> = manga_informations
                        .iter()
                        .filter_map(|info| {
                            info.cover_url
                                .as_ref()
                                .map(|url| (info.id.clone(), url.clone()))
                        })
                        .collect();
                    stream::iter(poster_items)
                        .map(|(id, url)| {
                            let chapter_storage = chapter_storage.clone();
                            let source = source.clone();
                            let token = token.clone();
                            async move {
                                let _ = timeout(
                                    Duration::from_secs(POSTER_DOWNLOAD_TIMEOUT_SECS),
                                    chapter_storage.cached_poster(&token, &id, || {
                                        source.get_image_request(url.clone(), None)
                                    }),
                                )
                                .await;
                            }
                        })
                        .buffered(CONCURRENT_POSTER_DOWNLOADS)
                        .collect::<Vec<_>>()
                        .await;
                }

                // Fetch unread chapters count for each manga
                let manga_ids: Vec<_> = manga_informations.iter().map(|m| m.id.clone()).collect();
                let unread_counts_map = db
                    .fetch_unread_chapter_counts_minimal(&manga_ids)
                    .await
                    .unwrap_or_default();
                let mangas: Vec<_> = manga_informations
                    .into_iter()
                    .map(move |manga| {
                        let unread_count = unread_counts_map.get(&manga.id).copied();
                        (manga, unread_count)
                    })
                    .collect();

                (
                    SourceMangaSearchResults {
                        source_information: source.manifest().into(),
                        mangas,
                    },
                    error,
                    has_next,
                    observation,
                )
            }
        })
        .buffered(CONCURRENT_SEARCH_REQUESTS)
        .collect::<Vec<_>>()
        .await;

    let mut outcomes = Vec::new();
    let mut has_next_page = false;
    let mut observations = Vec::new();
    let mut mangas: Vec<_> = source_results
        .into_iter()
        .flat_map(|(results, error, has_next, observation)| {
            if let Some(observation) = observation {
                observations.push(observation);
            }
            if has_next {
                has_next_page = true;
            }

            let SourceMangaSearchResults {
                mangas,
                source_information,
            } = results;

            outcomes.push(SourceSearchOutcome {
                source_id: source_information.id.value().clone(),
                source_name: source_information.name.clone(),
                result_count: mangas.len(),
                has_next_page: has_next,
                error,
            });

            mangas.into_iter().map(move |(manga, option_tuple)| {
                let (unread_count, last_read, in_library) =
                    option_tuple.unwrap_or((None, None, false));

                Manga {
                    source_information: source_information.clone(),
                    information: manga,
                    state: MangaState::default(),
                    unread_chapters_count: unread_count,
                    last_read,
                    in_library,
                    state_viewer: false,
                }
            })
        })
        .collect();

    if let Err(error) = source_health.record_batch(observations).await {
        warn!("couldn't persist search health: {error:#}");
    }

    mangas.sort_by_cached_key(|manga| {
        (
            manga
                .source_information
                .name
                .as_str()
                .nfkc()
                .flat_map(char::to_lowercase)
                .collect::<String>(),
            manga.source_information.id.value().clone(),
            manga
                .information
                .title
                .clone()
                .unwrap_or_default()
                .nfkc()
                .flat_map(char::to_lowercase)
                .collect::<String>(),
        )
    });
    outcomes.sort_by_cached_key(|outcome| {
        (
            outcome
                .source_name
                .as_str()
                .nfkc()
                .flat_map(char::to_lowercase)
                .collect::<String>(),
            outcome.source_id.clone(),
        )
    });

    Ok((mangas, outcomes, has_next_page))
}

fn source_is_included(included_source_ids: &Option<HashSet<String>>, source_id: &str) -> bool {
    included_source_ids
        .as_ref()
        .is_none_or(|included| included.contains(source_id))
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("an error occurred while fetching search results from the source")]
    SourceError(#[source] anyhow::Error),
}

type ResultManga = (MangaInformation, Option<(Option<usize>, Option<i64>, bool)>);
struct SourceMangaSearchResults {
    source_information: SourceInformation,
    /// mangas: Vec<Manga>, $0 is unread chapters count, $1 is last read time
    mangas: Vec<ResultManga>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_included_set_searches_every_source() {
        assert!(source_is_included(&None, "source.one"));
    }

    #[test]
    fn explicit_included_set_searches_only_exact_ids() {
        let included = Some(HashSet::from(["source.one".to_owned()]));

        assert!(source_is_included(&included, "source.one"));
        assert!(!source_is_included(&included, "source.two"));
    }

    #[test]
    fn explicit_empty_included_set_searches_nothing() {
        assert!(!source_is_included(&Some(HashSet::new()), "source.one"));
    }
}
