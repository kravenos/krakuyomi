use shared::{
    chapter_downloader::{
        ensure_chapter_is_in_storage, DownloadError, Error as ChapterDownloaderError,
    },
    chapter_storage::ChapterStorage,
    database::Database,
    model::ChapterId,
    settings::ChapterTitleFormat,
    source_collection::SourceCollection,
    source_manager::SourceManager,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{watch, Semaphore};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::{AppError, ErrorResponse};

use super::state::{Job, JobState};

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "type")]
pub enum Progress {
    Initializing,
    Downloading { processed: u32, total: u32 },
}

type JobSender =
    watch::Sender<Option<Result<Arc<(PathBuf, Vec<DownloadError>, bool)>, ErrorResponse>>>;
type JobReceiver =
    watch::Receiver<Option<Result<Arc<(PathBuf, Vec<DownloadError>, bool)>, ErrorResponse>>>;

type ProgressSender = watch::Sender<Progress>;
type ProgressReceiver = watch::Receiver<Progress>;

pub struct DownloadChapterJob {
    tx: JobSender,
    rx: JobReceiver,
    progress_rx: ProgressReceiver,
    handle: JoinHandle<()>,
    cancellation_token: CancellationToken,
}

impl DownloadChapterJob {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_new(
        source_manager: Arc<tokio::sync::Mutex<SourceManager>>,
        db: Arc<Database>,
        chapter_storage: ChapterStorage,
        chapter_id: ChapterId,
        concurrent_requests_pages: usize,
        optimize_image: bool,
        download_semaphore: Arc<Semaphore>,
        use_ram: bool,
        current_chapter_id: Option<ChapterId>,
        chapter_title_format: ChapterTitleFormat,
    ) -> Self {
        let (tx, rx) = watch::channel::<
            Option<Result<Arc<(PathBuf, Vec<DownloadError>, bool)>, ErrorResponse>>,
        >(None);

        let (progress_tx, progress_rx) = watch::channel(Progress::Initializing);

        let cancellation_token = CancellationToken::new();
        let tx_clone = tx.clone();
        let progress_tx_clone = progress_tx.clone();
        let token_clone = cancellation_token.clone();
        let handle = tokio::spawn(async move {
            let _permit = download_semaphore.acquire().await;
            let result = Self::do_job(
                token_clone,
                source_manager,
                db,
                chapter_storage,
                chapter_id,
                concurrent_requests_pages,
                optimize_image,
                progress_tx_clone,
                use_ram,
                current_chapter_id,
                chapter_title_format,
            )
            .await
            .map(Arc::new);

            let _ = tx_clone.send_replace(Some(result));
        });

        Self {
            tx,
            rx,
            progress_rx,
            handle,
            cancellation_token,
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn do_job(
        cancellation_token: CancellationToken,
        source_manager: Arc<tokio::sync::Mutex<SourceManager>>,
        db: Arc<Database>,
        chapter_storage: ChapterStorage,
        chapter_id: ChapterId,
        concurrent_requests_pages: usize,
        optimize_image: bool,
        progress_tx: ProgressSender,
        use_ram: bool,
        current_chapter_id: Option<ChapterId>,
        chapter_title_format: ChapterTitleFormat,
    ) -> Result<(PathBuf, Vec<DownloadError>, bool), ErrorResponse> {
        if use_ram {
            if let Some((path, errors)) = chapter_storage
                .get_stored_chapter_and_errors(&chapter_id, true)
                .map_err(AppError::from)?
            {
                return Ok((path, errors.unwrap_or_default(), true));
            }
        }
        if let Some((path, errors)) = chapter_storage
            .get_stored_chapter_and_errors(&chapter_id, false)
            .map_err(AppError::from)?
        {
            return Ok((path, errors.unwrap_or_default(), false));
        }

        let source = {
            let mgr = source_manager.lock().await;
            mgr.get_by_id(chapter_id.source_id())
                .cloned()
                .ok_or(AppError::SourceNotFound)?
        };

        let (manga, chapter) = {
            let manga = db
                .find_cached_manga_information(chapter_id.manga_id())
                .await
                .map_err(|e| ErrorResponse {
                    message: format!("Failed to fetch manga: {e}"),
                    code: None,
                    retryable: false,
                })?
                .ok_or_else(|| ErrorResponse {
                    message: "Manga not found in database".into(),
                    code: None,
                    retryable: false,
                })?;
            let chapter = db
                .find_cached_chapter_information(&chapter_id)
                .await
                .map_err(|e| ErrorResponse {
                    message: format!("Failed to fetch chapter: {e}"),
                    code: None,
                    retryable: false,
                })?
                .ok_or_else(|| ErrorResponse {
                    message: "Chapter not found in database".into(),
                    code: None,
                    retryable: false,
                })?;
            (manga, chapter)
        };

        let progress_callback = {
            let progress_tx = progress_tx.clone();
            Arc::new(move |processed: f32, total: f32| {
                let _ = progress_tx.send(Progress::Downloading {
                    processed: processed as u32,
                    total: total as u32,
                });
            })
        };

        let ((path, errors), chapter_use_ram) = match ensure_chapter_is_in_storage(
            &cancellation_token,
            &chapter_storage,
            &source,
            &manga,
            &chapter,
            concurrent_requests_pages,
            optimize_image,
            Some(progress_callback.clone()),
            use_ram,
            current_chapter_id.as_ref(),
            chapter_title_format,
        )
        .await
        {
            Ok(v) => (v, use_ram),
            Err(_)
                if use_ram
                    && chapter_storage
                        .tmpfs_full_storage()
                        .await
                        .unwrap_or_default() =>
            {
                (
                    ensure_chapter_is_in_storage(
                        &cancellation_token,
                        &chapter_storage,
                        &source,
                        &manga,
                        &chapter,
                        concurrent_requests_pages,
                        optimize_image,
                        Some(progress_callback),
                        false,
                        current_chapter_id.as_ref(),
                        chapter_title_format,
                    )
                    .await
                    .map_err(|e| {
                        let app_error = match e {
                            ChapterDownloaderError::DownloadError(e) => AppError::NetworkFailure(e),
                            ChapterDownloaderError::Other(e) => AppError::Other(e),
                        };
                        ErrorResponse::from(app_error)
                    })?,
                    false,
                )
            }
            Err(e) => {
                let app_error = match e {
                    ChapterDownloaderError::DownloadError(e) => AppError::NetworkFailure(e),
                    ChapterDownloaderError::Other(e) => AppError::Other(e),
                };
                return Err(ErrorResponse::from(app_error));
            }
        };

        Ok((path, errors, chapter_use_ram))
    }
}

impl Job for DownloadChapterJob {
    type Progress = Progress;
    type Output = Arc<(PathBuf, Vec<DownloadError>, bool)>;
    type Error = ErrorResponse;

    async fn cancel(&self) -> Result<(), AppError> {
        self.cancellation_token.cancel();
        self.handle.abort();

        let _ = self.tx.send(Some(Err(ErrorResponse {
            message: "Download was canceled by user".into(),
            code: None,
            retryable: false,
        })));

        Ok(())
    }

    async fn poll(&self) -> JobState<Self::Progress, Self::Output, Self::Error> {
        match self.rx.borrow().as_ref() {
            None => JobState::InProgress(*self.progress_rx.borrow()),
            Some(Ok(path)) => JobState::Completed(path.clone()),
            Some(Err(e)) => JobState::Errored(e.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use shared::{model::ChapterId, settings::Settings};
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn stored_chapter_opens_without_a_loaded_source() {
        let directory = tempdir().expect("create test directory");
        let chapter_id = ChapterId::from_strings(
            "missing.source".to_owned(),
            "manga".to_owned(),
            "chapter".to_owned(),
        );
        let storage = ChapterStorage::new(
            directory.path().join("downloads"),
            size::Size::from_bytes(1024 * 1024),
            false,
        )
        .expect("create chapter storage");
        let stored_path = storage.get_path_to_store_chapter(&chapter_id, false, false);
        std::fs::write(&stored_path, b"stored chapter").expect("write stored chapter");
        let source_manager = Arc::new(tokio::sync::Mutex::new(
            SourceManager::from_folder(directory.path().join("sources"), Settings::default())
                .expect("create source manager"),
        ));
        let database = Arc::new(
            Database::new(&directory.path().join("database.sqlite"))
                .await
                .expect("create database"),
        );
        let (progress_tx, _) = watch::channel(Progress::Initializing);

        let result = DownloadChapterJob::do_job(
            CancellationToken::new(),
            source_manager,
            database,
            storage,
            chapter_id,
            1,
            false,
            progress_tx,
            false,
            None,
            ChapterTitleFormat::default(),
        )
        .await;
        assert!(
            result.is_ok(),
            "stored chapter should open without a source"
        );
        let (actual_path, errors, used_ram) = result.ok().expect("checked successful result");

        assert_eq!(actual_path, stored_path);
        assert!(errors.is_empty());
        assert!(!used_ram);
    }
}
