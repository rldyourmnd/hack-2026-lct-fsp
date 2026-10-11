//! GitHub transport without a token: the public REST API, GitHub search and raw file downloads.
//!
//! Anonymous limits are per server IP: 60 REST requests per hour and 10 search requests per minute
//! ([rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)).
//! The client tracks both from response headers and waits for the reset instead of failing, so a
//! background analysis finishes later rather than with gaps. An optional token only raises limits.

use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;
use tokio::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    NotFound,
    /// The limit resets later than the caller may wait.
    RateLimited {
        retry_after_seconds: u64,
    },
    Unavailable,
}

#[derive(Clone, Copy)]
enum Bucket {
    Core,
    Search,
}

#[derive(Default)]
struct Budget {
    remaining: Option<u32>,
    reset_at: Option<Instant>,
    /// Earliest time for the next request: spaces anonymous search to ten per minute.
    next_at: Option<Instant>,
}

pub struct GitHubClient {
    http: Client,
    token: Option<String>,
    api_url: String,
    raw_url: String,
    core: Arc<Mutex<Budget>>,
    search: Arc<Mutex<Budget>>,
    /// Longest wait for a rate-limit reset inside one request.
    max_wait: Duration,
    /// Spacing between search requests; `None` keeps GitHub's documented rate.
    search_interval: Option<Duration>,
}

impl GitHubClient {
    pub fn new(token: Option<String>) -> Result<Self, reqwest::Error> {
        Self::with_endpoints(
            token,
            "https://api.github.com".into(),
            "https://raw.githubusercontent.com".into(),
        )
    }

    pub fn with_endpoints(
        token: Option<String>,
        api_url: String,
        raw_url: String,
    ) -> Result<Self, reqwest::Error> {
        let http = Client::builder()
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::limited(3))
            .user_agent("fsp-github-insights/0.2")
            .build()?;
        Ok(Self {
            http,
            token,
            api_url,
            raw_url,
            core: Arc::default(),
            search: Arc::default(),
            max_wait: Duration::from_secs(70),
            search_interval: None,
        })
    }

    /// Overrides the spacing between search requests (for a local fake of GitHub).
    #[must_use]
    pub fn with_search_interval(mut self, interval: Duration) -> Self {
        self.search_interval = Some(interval);
        self
    }

    pub fn authenticated(&self) -> bool {
        self.token.is_some()
    }

    /// GETs a REST resource.
    pub async fn rest<T: DeserializeOwned>(&self, path: &str) -> Result<T, FetchError> {
        self.json(Bucket::Core, path).await
    }

    /// GETs a search resource (`/search/...`), respecting the search budget.
    pub async fn search<T: DeserializeOwned>(&self, path: &str) -> Result<T, FetchError> {
        self.json(Bucket::Search, path).await
    }

    /// Downloads a public file of a repository's default branch; `None` when absent or too large.
    pub async fn raw_file(&self, full_name: &str, path: &str) -> Option<String> {
        let url = format!("{}/{full_name}/HEAD/{path}", self.raw_url);
        let response = self.http.get(url).send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        if response
            .content_length()
            .is_some_and(|length| length > 256 * 1024)
        {
            return None;
        }
        let text = response.text().await.ok()?;
        (text.len() <= 256 * 1024).then_some(text)
    }

    async fn json<T: DeserializeOwned>(&self, bucket: Bucket, path: &str) -> Result<T, FetchError> {
        let budget = match bucket {
            Bucket::Core => &self.core,
            Bucket::Search => &self.search,
        };
        for attempt in 0..3 {
            self.wait_for(bucket, budget).await?;
            let mut request = self
                .http
                .get(format!("{}{path}", self.api_url))
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28");
            if let Some(token) = &self.token {
                request = request.bearer_auth(token);
            }
            let response = match request.send().await {
                Ok(response) => response,
                Err(_) if attempt < 2 => continue,
                Err(_) => return Err(FetchError::Unavailable),
            };
            record(budget, &response).await;
            match response.status() {
                status if status.is_success() => {
                    return response.json().await.map_err(|_| FetchError::Unavailable);
                }
                StatusCode::NOT_FOUND | StatusCode::GONE => return Err(FetchError::NotFound),
                StatusCode::UNPROCESSABLE_ENTITY => return Err(FetchError::NotFound),
                StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => {
                    // A primary limit is recorded above; a secondary one waits a minute.
                    let retry = retry_after(&response).unwrap_or(60);
                    let mut state = budget.lock().await;
                    state.next_at = Some(Instant::now() + Duration::from_secs(retry));
                    if state.reset_at.is_none() {
                        state.remaining = Some(0);
                        state.reset_at = state.next_at;
                    }
                }
                status if status.is_server_error() && attempt < 2 => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                _ => return Err(FetchError::Unavailable),
            }
        }
        Err(FetchError::Unavailable)
    }

    async fn wait_for(&self, bucket: Bucket, budget: &Mutex<Budget>) -> Result<(), FetchError> {
        let mut state = budget.lock().await;
        let now = Instant::now();
        let mut until = state.next_at.filter(|at| *at > now);
        if state.remaining == Some(0)
            && let Some(reset) = state.reset_at.filter(|at| *at > now)
        {
            until = Some(until.map_or(reset, |at| at.max(reset)));
        }
        if let Some(until) = until {
            let wait = until - now;
            if wait > self.max_wait {
                return Err(FetchError::RateLimited {
                    retry_after_seconds: wait.as_secs().max(1),
                });
            }
            tokio::time::sleep(wait).await;
            state.remaining = None;
        }
        // Anonymous search allows ten requests a minute; authenticated search thirty.
        if matches!(bucket, Bucket::Search) {
            let spacing =
                self.search_interval
                    .unwrap_or(Duration::from_millis(if self.token.is_some() {
                        2_100
                    } else {
                        6_200
                    }));
            state.next_at = Some(Instant::now() + spacing);
        }
        Ok(())
    }
}

async fn record(budget: &Mutex<Budget>, response: &Response) {
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
    };
    let mut state = budget.lock().await;
    if let Some(remaining) = header("x-ratelimit-remaining") {
        state.remaining = u32::try_from(remaining).ok();
    }
    if let Some(reset) = header("x-ratelimit-reset") {
        let now = chrono::Utc::now().timestamp().max(0) as u64;
        state.reset_at = Some(Instant::now() + Duration::from_secs(reset.saturating_sub(now) + 1));
    }
}

fn retry_after(response: &Response) -> Option<u64> {
    response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
}

/// Percent-encodes a search query for a URL.
pub fn encode_query(query: &str) -> String {
    let mut encoded = String::with_capacity(query.len());
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_queries_are_encoded() {
        assert_eq!(
            encode_query("type:pr author:a-b -org:x review:approved"),
            "type%3Apr+author%3Aa-b+-org%3Ax+review%3Aapproved"
        );
        assert_eq!(
            encode_query("author-date:>=2026-01-01"),
            "author-date%3A%3E%3D2026-01-01"
        );
    }
}
