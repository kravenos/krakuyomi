use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;

use crate::{
    database::Database,
    model::{ChapterInformation, MangaId},
    source::Source,
    source_health::SourceOperationError,
};

pub async fn refresh_manga_chapters<'a>(
    token: &CancellationToken,
    db: &'a Database,
    source: &'a Source,
    id: &'a MangaId,
    seconds: u64,
) -> Result<Vec<ChapterInformation>, SourceOperationError> {
    let duration = Duration::from_secs(seconds);
    let child_token = token.child_token();

    let fetch_task = async {
        source
            .get_chapter_list(child_token.clone(), id.value().clone())
            .await
    };

    let fresh_chapter_informations = match timeout(duration, fetch_task).await {
        Ok(Ok(list)) => list.into_iter().map(From::from).collect::<Vec<_>>(),

        Ok(Err(error)) => return Err(SourceOperationError::classify(error)),

        Err(_) => {
            child_token.cancel();
            return Err(SourceOperationError::timeout());
        }
    };

    db.upsert_cached_chapter_informations(id, &fresh_chapter_informations)
        .await
        .map_err(anyhow::Error::from)
        .map_err(SourceOperationError::classify)?;

    Ok(fresh_chapter_informations)
}
