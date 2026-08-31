use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;

use crate::{
    chapter_storage::ChapterStorage,
    database::Database,
    model::MangaId,
    source::{model::PublishingStatus, Source},
    source_health::SourceOperationError,
};

pub async fn refresh_manga_details(
    token: &CancellationToken,
    db: &Database,
    chapter_storage: &ChapterStorage,
    source: &Source,
    id: &MangaId,
    seconds: u64,
) -> Result<PublishingStatus, SourceOperationError> {
    let duration = Duration::from_secs(seconds);

    let child_token = token.child_token();

    let fetch_task = async {
        source
            .get_manga_details(child_token.clone(), id.value().clone())
            .await
    };

    let manga_details = match timeout(duration, fetch_task).await {
        Ok(Ok(manga)) => manga,

        Ok(Err(error)) => return Err(SourceOperationError::classify(error)),

        Err(_) => {
            child_token.cancel();
            return Err(SourceOperationError::timeout());
        }
    };

    db.upsert_cached_manga_details(id, &manga_details)
        .await
        .map_err(anyhow::Error::from)
        .map_err(SourceOperationError::classify)?;

    if let Some(url) = &manga_details.cover_url {
        chapter_storage
            .cached_poster(token, id, || source.get_image_request(url.to_owned(), None))
            .await
            .map_err(SourceOperationError::classify)?;
    }

    Ok(manga_details.status)
}
