//! HTTP Range request support for fetching remote CSV files.

use std::io::Read;
use std::time::Duration;
use thiserror::Error;

/// Default timeout for HTTP requests (30 seconds).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Result of fetching a URL.
pub struct FetchResult {
    /// The fetched data bytes.
    pub data: Vec<u8>,
    /// Whether the server supports Range requests.
    #[allow(dead_code)]
    pub range_supported: bool,
    /// Total content length if known.
    #[allow(dead_code)]
    pub content_length: Option<u64>,
}

/// Errors that can occur during HTTP fetching.
#[derive(Error, Debug)]
pub enum HttpError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("HTTP error {status}: {message}")]
    HttpStatus { status: u16, message: String },
    #[error("Network error: {0}")]
    Network(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<ureq::Error> for HttpError {
    fn from(err: ureq::Error) -> Self {
        match err {
            ureq::Error::StatusCode(code) => HttpError::HttpStatus {
                status: code,
                message: format!("Server returned status {code}"),
            },
            _ => HttpError::Network(err.to_string()),
        }
    }
}

/// Fetch data from a URL, optionally using Range requests to limit download size.
///
/// If `max_bytes` is `Some(n)`, attempts a Range request for the first `n` bytes.
/// Falls back to full download (truncated at max_bytes) if the server doesn't support Range requests.
pub fn fetch_url(url: &str, max_bytes: Option<usize>) -> Result<FetchResult, HttpError> {
    // Validate URL
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(HttpError::InvalidUrl(format!(
            "URL must start with http:// or https://: {url}"
        )));
    }

    // Try Range request if max_bytes is specified
    if let Some(bytes) = max_bytes {
        match fetch_with_range(url, bytes) {
            Ok(result) if result.range_supported => return Ok(result),
            Ok(result) => {
                // Server responded with 200 instead of 206 - it doesn't support Range
                // The result already contains truncated data
                return Ok(result);
            }
            Err(HttpError::HttpStatus { status: 416, .. }) => {
                // Range Not Satisfiable - file might be smaller than requested range
                // Fall through to full download
            }
            Err(e) => return Err(e),
        }
    }

    // Full download (no Range request)
    fetch_full(url, max_bytes)
}

/// Attempt to fetch with a Range request.
fn fetch_with_range(url: &str, bytes: usize) -> Result<FetchResult, HttpError> {
    let range_header = format!("bytes=0-{}", bytes.saturating_sub(1));

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(DEFAULT_TIMEOUT))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let response = agent.get(url).header("Range", &range_header).call()?;

    let status = response.status();
    let content_length = response
        .headers()
        .get("Content-Range")
        .and_then(|h| {
            // Parse "bytes 0-N/TOTAL" format
            let s = h.to_str().ok()?;
            s.split('/').next_back()?.parse::<u64>().ok()
        })
        .or_else(|| {
            response
                .headers()
                .get("Content-Length")
                .and_then(|h| h.to_str().ok()?.parse::<u64>().ok())
        });

    // 206 Partial Content means Range was accepted
    let range_supported = status == 206;

    // Read the body - use take() to truncate instead of erroring
    let body = response.into_body();
    let reader = body.into_reader();
    let mut data = Vec::with_capacity(bytes);
    reader.take(bytes as u64).read_to_end(&mut data)?;

    Ok(FetchResult {
        data,
        range_supported,
        content_length,
    })
}

/// Fetch the full content (or up to max_bytes if specified).
fn fetch_full(url: &str, max_bytes: Option<usize>) -> Result<FetchResult, HttpError> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(DEFAULT_TIMEOUT))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let response = agent.get(url).call()?;

    let content_length = response
        .headers()
        .get("Content-Length")
        .and_then(|h| h.to_str().ok()?.parse::<u64>().ok());

    let body = response.into_body();
    let mut reader = body.into_reader();

    let data = if let Some(bytes) = max_bytes {
        let mut buf = Vec::with_capacity(bytes);
        reader.take(bytes as u64).read_to_end(&mut buf)?;
        buf
    } else {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        buf
    };

    Ok(FetchResult {
        data,
        range_supported: false,
        content_length,
    })
}
