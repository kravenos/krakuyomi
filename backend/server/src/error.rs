use std::path::PathBuf;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use log::error;
use serde::Serialize;

use shared::source_health::SourceOperationError;
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
    Conflict(String),
    DownloadAllChaptersProgressNotFound,
    SourceCatalog(anyhow::Error),
    SourceStatus(anyhow::Error),
    SourceOperation(SourceOperationError),
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
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::SourceCatalog(_) => StatusCode::BAD_GATEWAY,
            AppError::SourceStatus(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::SourceOperation(error)
                if error.category() == shared::source_health::SourceErrorCategory::Timeout =>
            {
                StatusCode::GATEWAY_TIMEOUT
            }
            AppError::SourceOperation(_) => StatusCode::BAD_GATEWAY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<&AppError> for ErrorResponse {
    fn from(value: &AppError) -> Self {
        let message = match value {
            AppError::SourceNotFound => "Source was not found".to_string(),
            AppError::NotFound => "Requested item was not found".to_string(),
            AppError::Conflict(message) => message.clone(),
            AppError::DownloadAllChaptersProgressNotFound => {
                "No download is in progress.".to_string()
            }
            AppError::SourceCatalog(_) => {
                "The source list could not be validated or refreshed.".to_owned()
            }
            AppError::SourceStatus(_) => "Source status could not be loaded.".to_owned(),
            AppError::SourceOperation(error) => error.safe_message().to_owned(),
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
            AppError::Conflict(_) => (Some("stale_uninstall_preview".to_owned()), false),
            AppError::SourceCatalog(_) => (Some("source_catalog_unavailable".to_owned()), true),
            AppError::SourceStatus(_) => (Some("source_status_unavailable".to_owned()), true),
            AppError::SourceOperation(error) => (
                Some(error.category().code().to_owned()),
                error.category().retryable(),
            ),
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
            Self::SourceCatalog(ref e) => Some(e),
            Self::SourceStatus(ref e) => Some(e),
            Self::SourceOperation(ref error) => Some(error.cause()),
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

    #[test]
    fn uninstall_conflict_has_stable_code() {
        let response = ErrorResponse::from(&AppError::Conflict("review again".to_owned()));

        assert_eq!(response.code.as_deref(), Some("stale_uninstall_preview"));
        assert!(!response.retryable);
    }

    #[test]
    fn source_operation_error_exposes_only_the_safe_category() {
        let response =
            ErrorResponse::from(&AppError::SourceOperation(SourceOperationError::classify(
                anyhow::anyhow!("WASM trap at /private/path with cookie=secret"),
            )));

        assert_eq!(response.code.as_deref(), Some("source_trap"));
        assert_eq!(
            response.message,
            "The source stopped while processing the request."
        );
        assert!(!response.message.contains("private"));
        assert!(!response.message.contains("secret"));
        assert!(response.retryable);
    }

    #[test]
    fn source_status_error_does_not_expose_private_database_details() {
        let response = ErrorResponse::from(&AppError::SourceStatus(anyhow::anyhow!(
            "database at /private/path failed with token=secret"
        )));

        assert_eq!(response.message, "Source status could not be loaded.");
        assert_eq!(response.code.as_deref(), Some("source_status_unavailable"));
        assert!(response.retryable);
        assert!(!response.message.contains("private"));
        assert!(!response.message.contains("secret"));
    }

    #[test]
    fn source_catalog_error_does_not_expose_private_url_details() {
        let response = ErrorResponse::from(&AppError::SourceCatalog(anyhow::anyhow!(
            "https://user:secret@example.com/index.json?token=private"
        )));

        assert_eq!(
            response.message,
            "The source list could not be validated or refreshed."
        );
        assert_eq!(response.code.as_deref(), Some("source_catalog_unavailable"));
        assert!(response.retryable);
        assert!(!response.message.contains("secret"));
        assert!(!response.message.contains("private"));
    }
}
