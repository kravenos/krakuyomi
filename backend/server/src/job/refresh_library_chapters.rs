use std::{collections::BTreeMap, sync::Arc};

use futures::lock::Mutex;
use serde::Serialize;
use shared::{
    database::Database,
    source_collection::SourceCollection,
    source_health::{
        SourceErrorCategory, SourceHealthBatch, SourceHealthObservation, SourceHealthStore,
        SourceOperationClass, SourceOperationReport, SourceOperationSummary,
    },
    source_manager::SourceManager,
    usecases::{self, get_manga_library},
};
use tokio_util::sync::CancellationToken;

use crate::ErrorResponse;

use super::state::{Job, JobState};

#[derive(Default)]
enum Status {
    #[default]
    Initializing,
    Progressing {
        current: usize,
        total: usize,
        report: SourceOperationReport,
    },
    Finished(SourceOperationReport),
    Errored(String),
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "type")]
pub enum Progress {
    Initializing,
    Refreshing {
        current: usize,
        total: usize,
        report: SourceOperationReport,
    },
}

pub struct RefreshLibraryChaptersJob {
    cancellation_token: CancellationToken,
    status: Arc<Mutex<Status>>,
}

impl RefreshLibraryChaptersJob {
    pub fn spawn_new(
        source_manager: Arc<tokio::sync::Mutex<SourceManager>>,
        database: Arc<Database>,
        source_health: SourceHealthStore,
    ) -> Self {
        let cancellation_token = CancellationToken::new();
        let cancellation_token_clone = cancellation_token.clone();

        let status: Arc<Mutex<Status>> = Default::default();
        let status_clone = status.clone();

        tokio::spawn(async move {
            let status = status_clone;
            let cancellation_token = cancellation_token_clone;

            let (mangas, source_manager) = {
                let sm = source_manager.lock().await;
                let mangas = match get_manga_library(
                    &database,
                    &*sm,
                    &shared::settings::LibrarySortingMode::TitleAsc,
                )
                .await
                {
                    Ok(m) => m,
                    Err(e) => {
                        *status.lock().await = Status::Errored(e.to_string());
                        return;
                    }
                };
                (mangas, (*sm).clone())
            };

            let total = mangas.len();
            let mut summaries = BTreeMap::<String, SourceOperationSummary>::new();
            let mut observations = SourceHealthBatch::default();
            for (i, manga) in mangas.into_iter().enumerate() {
                if cancellation_token.is_cancelled() {
                    break;
                }

                {
                    let mut status_lock = status.lock().await;
                    match &mut *status_lock {
                        Status::Initializing => {
                            *status_lock = Status::Progressing {
                                current: i,
                                total,
                                report: SourceOperationReport::default(),
                            };
                        }
                        Status::Progressing { current, .. } => {
                            *current = i;
                        }
                        _ => {}
                    }
                }

                let manga_id = manga.information.id;
                let source_id = manga_id.source_id().value().clone();
                summaries.entry(source_id.clone()).or_insert_with(|| {
                    SourceOperationSummary::new(
                        source_id.clone(),
                        manga.source_information.name.clone(),
                    )
                });
                let source = match source_manager.get_by_id(manga_id.source_id()) {
                    Some(s) => s,
                    None => {
                        summaries
                            .get_mut(&source_id)
                            .expect("source summary exists")
                            .record_skip(SourceErrorCategory::MissingSource);
                        observations.push(SourceHealthObservation::failure_with_category(
                            source_id,
                            SourceOperationClass::RefreshChapters,
                            manga_id.value(),
                            SourceErrorCategory::MissingSource,
                        ));
                        update_report(&status, i + 1, total, &summaries).await;
                        continue;
                    }
                };

                match usecases::refresh_manga_chapters(
                    &cancellation_token,
                    &database,
                    source,
                    &manga_id,
                    60,
                )
                .await
                {
                    Ok(_) => {
                        summaries
                            .get_mut(&source_id)
                            .expect("source summary exists")
                            .record_success();
                        observations.push(SourceHealthObservation::success(
                            source_id,
                            SourceOperationClass::RefreshChapters,
                            manga_id.value(),
                        ));
                    }
                    Err(error) => {
                        log::warn!(
                            "chapter refresh failed for source {}: {:#}",
                            source_id,
                            error.cause()
                        );
                        summaries
                            .get_mut(&source_id)
                            .expect("source summary exists")
                            .record_failure(manga_id.value(), &error);
                        observations.push(SourceHealthObservation::failure(
                            source_id,
                            SourceOperationClass::RefreshChapters,
                            manga_id.value(),
                            &error,
                        ));
                    }
                }
                update_report(&status, i + 1, total, &summaries).await;
            }

            if let Err(error) = source_health
                .record_batch(observations.into_observations())
                .await
            {
                log::warn!("couldn't persist chapter refresh health: {error:#}");
            }
            let report = report_from(&summaries);
            let mut status_lock = status.lock().await;
            if !matches!(*status_lock, Status::Errored(_)) {
                *status_lock = Status::Finished(report);
            }
        });

        Self {
            cancellation_token,
            status,
        }
    }
}

impl Job for RefreshLibraryChaptersJob {
    type Progress = Progress;
    type Output = SourceOperationReport;
    type Error = ErrorResponse;

    async fn cancel(&self) -> Result<(), crate::AppError> {
        self.cancellation_token.cancel();

        Ok(())
    }

    async fn poll(&self) -> JobState<Self::Progress, Self::Output, Self::Error> {
        let status = &*self.status.lock().await;

        match status {
            Status::Initializing => JobState::InProgress(Progress::Initializing),
            Status::Progressing {
                current,
                total,
                report,
            } => JobState::InProgress(Progress::Refreshing {
                current: *current,
                total: *total,
                report: report.clone(),
            }),
            Status::Finished(report) => JobState::Completed(report.clone()),
            Status::Errored(e) => {
                let error = crate::AppError::from(anyhow::anyhow!(e.to_string()));
                JobState::Errored(error.into())
            }
        }
    }
}

fn report_from(summaries: &BTreeMap<String, SourceOperationSummary>) -> SourceOperationReport {
    SourceOperationReport {
        summaries: summaries.values().cloned().collect(),
    }
}

async fn update_report(
    status: &Mutex<Status>,
    current: usize,
    total: usize,
    summaries: &BTreeMap<String, SourceOperationSummary>,
) {
    *status.lock().await = Status::Progressing {
        current,
        total,
        report: report_from(summaries),
    };
}
