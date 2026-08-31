use std::time::Duration;

use url::Url;

/// Safe, bounded evidence returned when probing a source-declared base URL.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseUrlProbe {
    /// The server returned a non-redirect HTTP response.
    Response(u16),
    /// The server returned a redirect. The target is deliberately not exposed.
    Redirect(u16),
    /// The source did not declare a usable HTTP(S) URL.
    Unsupported,
    /// The request exceeded its fixed deadline.
    TimedOut,
    /// The request failed before a response was received.
    Failed,
}

/// Probes only the response headers of one source-declared HTTP(S) URL.
///
/// Redirects are not followed, response bodies are not read, errors are reduced
/// to a safe category, and the whole request is bounded by `timeout_duration`.
pub async fn probe_base_url(value: &str, timeout_duration: Duration) -> BaseUrlProbe {
    let Ok(url) = Url::parse(value) else {
        return BaseUrlProbe::Unsupported;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return BaseUrlProbe::Unsupported;
    }

    let Ok(client) = crate::tls::client_builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
    else {
        return BaseUrlProbe::Failed;
    };
    let request = client.get(url).send();
    match tokio::time::timeout(timeout_duration, request).await {
        Ok(Ok(response)) if response.status().is_redirection() => {
            BaseUrlProbe::Redirect(response.status().as_u16())
        }
        Ok(Ok(response)) => BaseUrlProbe::Response(response.status().as_u16()),
        Ok(Err(_)) => BaseUrlProbe::Failed,
        Err(_) => BaseUrlProbe::TimedOut,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_non_web_urls_without_contacting_them() {
        assert_eq!(
            probe_base_url("file:///private/source.json", Duration::from_millis(1)).await,
            BaseUrlProbe::Unsupported
        );
        assert_eq!(
            probe_base_url("not a URL", Duration::from_millis(1)).await,
            BaseUrlProbe::Unsupported
        );
    }
}
