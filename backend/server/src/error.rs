use std::path::PathBuf;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use log::error;
use serde::Serialize;

use shared::usecases::{
    fetch_manga_chapter::Error as FetchMangaChaptersError,
    search_mangas::Error as SearchMangasError,
};

pub(crate) fn setcap_hint() -> String {
    let bin_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("./server"));
    let display = bin_path.display();
    format!(
        "\n\nHint: Run the following command to grant mount capability:\n  sudo setcap cap_sys_admin+ep {display}\n\nThen restart the server."
    )
}

pub enum AppError {
    SourceNotFound,
    NotFound,
    DownloadAllChaptersProgressNotFound,
    NetworkFailure(anyhow::Error),
    Other(anyhow::Error),
    MountTmpFs(anyhow::Error),
}

#[derive(Serialize, Clone)]
pub struct ErrorResponse {
    /// Safe user-facing summary.
    pub message: String,
    /// Stable machine-readable category when one is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Whether repeating the same request may succeed without user changes.
    pub retryable: bool,
}

impl AppError {
    pub fn from_search_mangas_error(value: SearchMangasError) -> Self {
        match value {
            SearchMangasError::SourceError(e) => Self::NetworkFailure(e),
        }
    }

    pub fn from_fetch_manga_chapters_error(value: FetchMangaChaptersError) -> Self {
        match value {
            FetchMangaChaptersError::DownloadError(e) => Self::NetworkFailure(e),
            FetchMangaChaptersError::Other(e) => Self::Other(e),
        }
    }
}

impl From<&AppError> for StatusCode {
    fn from(value: &AppError) -> Self {
        match &value {
            AppError::SourceNotFound
            | AppError::NotFound
            | AppError::DownloadAllChaptersProgressNotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<&AppError> for ErrorResponse {
    fn from(value: &AppError) -> Self {
        let message = match value {
            AppError::SourceNotFound => "Source was not found".to_string(),
            AppError::NotFound => "Requested item was not found".to_string(),
            AppError::DownloadAllChaptersProgressNotFound => {
                "No download is in progress.".to_string()
            }
            AppError::NetworkFailure(e) => {
                eprintln!("Networke error: {:?}", e);
                format!(
                    "There was a network error. Check your connection and try again ({:?})",
                    e
                )
            }
            AppError::MountTmpFs(ref e) => format!("Failed to mount tmpfs: {}{}", e, setcap_hint()),
            AppError::Other(ref e) => {
                eprintln!("Unexpected error: {:?}", e);

                format!("Something went wrong: {}", e)
            }
        };

        let (code, retryable) = match value {
            AppError::SourceNotFound => (Some("missing_source".to_owned()), false),
            AppError::NetworkFailure(_) => (Some("network_failure".to_owned()), true),
            _ => (None, false),
        };

        Self {
            message,
            code,
            retryable,
        }
    }
}

impl From<AppError> for ErrorResponse {
    fn from(value: AppError) -> Self {
        Self::from(&value)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status_code = StatusCode::from(&self);
        let error_response = ErrorResponse::from(&self);

        let inner_exception = match self {
            Self::NetworkFailure(ref e) => Some(e),
            Self::Other(ref e) => Some(e),
            _ => None,
        };

        if let Some(e) = inner_exception {
            error!("Error caused by: {:?}", e);
        }

        (status_code, Json(error_response)).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self::Other(err.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_source_error_has_stable_safe_fields() {
        let response = ErrorResponse::from(&AppError::SourceNotFound);

        assert_eq!(response.message, "Source was not found");
        assert_eq!(response.code.as_deref(), Some("missing_source"));
        assert!(!response.retryable);
    }

    #[test]
    fn network_error_is_marked_retryable() {
        let response = ErrorResponse::from(&AppError::NetworkFailure(anyhow::anyhow!(
            "temporary failure"
        )));

        assert_eq!(response.code.as_deref(), Some("network_failure"));
        assert!(response.retryable);
    }
}
