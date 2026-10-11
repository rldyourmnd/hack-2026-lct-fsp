//! Loopback HTTP API of the insights service.
//!
//! Analyses run in the background: `POST /v1/analyses` queues one (or returns a fresh result) and
//! `GET /v1/analyses/{login}` reports the stage, progress and finally the report. Two analyses run
//! at a time; finished reports stay cached for six hours, so repeated views never repeat the work.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use fsp_domain::{GitHubReport, parse_github_login};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::Instant;

use crate::analysis::build_report;
use crate::client::FetchError;
use crate::collect::{CollectError, Collector};

const FRESH: Duration = Duration::from_secs(6 * 60 * 60);
const FAILED_RETRY: Duration = Duration::from_secs(5 * 60);
/// A bio read more recently than this is reused for the ownership check; a candidate who just
/// added the code is checked against a fresh read.
const BIO_FRESH: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed,
}

struct Job {
    login: String,
    state: JobState,
    stage: String,
    progress: u8,
    finished: Option<Instant>,
    report: Option<Arc<GitHubReport>>,
    bio: Option<String>,
    bio_at: Option<Instant>,
    error: Option<(&'static str, &'static str)>,
    retry_after_seconds: Option<u64>,
}

pub struct InsightsState {
    collector: Option<Arc<Collector>>,
    jobs: Mutex<HashMap<String, Arc<Mutex<Job>>>>,
    workers: Arc<Semaphore>,
}

impl InsightsState {
    #[must_use]
    pub fn new(collector: Option<Collector>) -> Self {
        Self {
            collector: collector.map(Arc::new),
            jobs: Mutex::new(HashMap::new()),
            workers: Arc::new(Semaphore::new(2)),
        }
    }
}

pub fn router(state: Arc<InsightsState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/analyses", post(submit))
        .route("/v1/analyses/{login}", get(status))
        .with_state(state)
}

async fn health(State(state): State<Arc<InsightsState>>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "github": if state.collector.is_some() { "configured" } else { "unavailable" },
        "authenticated": state
            .collector
            .as_ref()
            .is_some_and(|collector| collector.client.authenticated()),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitRequest {
    login: String,
    ownership_code: Option<String>,
    #[serde(default)]
    refresh: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusQuery {
    ownership_code: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    login: String,
    state: JobState,
    stage: String,
    progress: u8,
    report: Option<GitHubReport>,
    ownership_code_found: Option<bool>,
    error: Option<serde_json::Value>,
    retry_after_seconds: Option<u64>,
}

async fn submit(
    State(state): State<Arc<InsightsState>>,
    Json(request): Json<SubmitRequest>,
) -> Result<Json<StatusResponse>, ServiceError> {
    let mut login = parse_github_login(&request.login).ok_or(ServiceError::InvalidLogin)?;
    let collector = state.collector.clone().ok_or(ServiceError::NotConfigured)?;
    let key = login.to_ascii_lowercase();
    let existing = state.jobs.lock().await.get(&key).cloned();
    if let Some(job) = existing {
        let reuse = {
            let job = job.lock().await;
            match job.state {
                JobState::Queued | JobState::Running => true,
                JobState::Done => {
                    !request.refresh && job.finished.is_some_and(|at| at.elapsed() < FRESH)
                }
                JobState::Failed => {
                    !request.refresh && job.finished.is_some_and(|at| at.elapsed() < FAILED_RETRY)
                }
            }
        };
        if reuse {
            return respond(&collector, &job, request.ownership_code.as_deref()).await;
        }
    }
    // One REST request rejects a mistyped login or an organization at once rather than after a
    // queued analysis; on a rate limit or outage the analysis itself retries in the background.
    match collector.profile(&login).await {
        Err(CollectError::UnknownAccount) => return Err(ServiceError::UnknownAccount),
        Ok((canonical, _)) => login = canonical,
        Err(_) => {}
    }
    let job = Arc::new(Mutex::new(Job {
        login: login.clone(),
        state: JobState::Queued,
        stage: "В очереди".into(),
        progress: 0,
        finished: None,
        report: None,
        bio: None,
        bio_at: None,
        error: None,
        retry_after_seconds: None,
    }));
    state.jobs.lock().await.insert(key, job.clone());
    tokio::spawn(run(
        collector.clone(),
        state.workers.clone(),
        job.clone(),
        login,
    ));
    respond(&collector, &job, request.ownership_code.as_deref()).await
}

async fn status(
    State(state): State<Arc<InsightsState>>,
    Path(login): Path<String>,
    Query(query): Query<StatusQuery>,
) -> Result<Json<StatusResponse>, ServiceError> {
    let login = parse_github_login(&login).ok_or(ServiceError::InvalidLogin)?;
    let collector = state.collector.clone().ok_or(ServiceError::NotConfigured)?;
    let job = state
        .jobs
        .lock()
        .await
        .get(&login.to_ascii_lowercase())
        .cloned()
        .ok_or(ServiceError::UnknownJob)?;
    respond(&collector, &job, query.ownership_code.as_deref()).await
}

async fn respond(
    collector: &Collector,
    job: &Mutex<Job>,
    code: Option<&str>,
) -> Result<Json<StatusResponse>, ServiceError> {
    let mut job = job.lock().await;
    let code = code.filter(|code| (8..=64).contains(&code.len()));
    let mut ownership_code_found = None;
    if job.state == JobState::Done
        && let Some(code) = code
    {
        if job.bio_at.is_none_or(|at| at.elapsed() > BIO_FRESH) {
            // One REST request: the bio may have changed since the analysis.
            if let Ok((_, bio)) = collector.profile(&job.login).await {
                job.bio = bio;
                job.bio_at = Some(Instant::now());
            }
        }
        ownership_code_found = Some(job.bio.as_deref().is_some_and(|bio| bio.contains(code)));
    }
    Ok(Json(StatusResponse {
        login: job.login.clone(),
        state: job.state,
        stage: job.stage.clone(),
        progress: job.progress,
        report: job.report.as_deref().cloned(),
        ownership_code_found,
        error: job
            .error
            .map(|(code, message)| json!({ "code": code, "message": message })),
        retry_after_seconds: job.retry_after_seconds,
    }))
}

async fn run(
    collector: Arc<Collector>,
    workers: Arc<Semaphore>,
    job: Arc<Mutex<Job>>,
    login: String,
) {
    let Ok(_permit) = workers.acquire_owned().await else {
        return;
    };
    let started = Instant::now();
    for attempt in 0..3 {
        {
            let mut job = job.lock().await;
            job.state = JobState::Running;
            job.retry_after_seconds = None;
        }
        let progress_job = job.clone();
        let progress = Arc::new(move |stage: &str, percent: u8| {
            if let Ok(mut job) = progress_job.try_lock() {
                job.stage = stage.to_owned();
                job.progress = percent.max(job.progress);
            }
        });
        match collector.collect(&login, progress).await {
            Ok(collected) => {
                let report = build_report(&collected.snapshot);
                tracing::info!(
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    repositories = collected.snapshot.analyzed.len(),
                    partial = collected.snapshot.partial,
                    "github report built"
                );
                let mut job = job.lock().await;
                job.state = JobState::Done;
                job.stage = "Готово".into();
                job.progress = 100;
                job.finished = Some(Instant::now());
                job.report = Some(Arc::new(report));
                job.bio = collected.bio;
                job.bio_at = Some(Instant::now());
                return;
            }
            Err(CollectError::Fetch(FetchError::RateLimited {
                retry_after_seconds,
            })) if attempt < 2 && retry_after_seconds <= 3600 => {
                // GitHub's anonymous limits reset within the hour: wait and continue.
                let mut state = job.lock().await;
                state.state = JobState::Queued;
                state.stage = "Ждём сброса лимита GitHub".into();
                state.retry_after_seconds = Some(retry_after_seconds);
                drop(state);
                tokio::time::sleep(Duration::from_secs(retry_after_seconds + 2)).await;
            }
            Err(error) => {
                let (code, message) = match error {
                    CollectError::UnknownAccount => {
                        ("github_account_not_found", "Пользователь GitHub не найден")
                    }
                    CollectError::Fetch(FetchError::RateLimited { .. }) => (
                        "github_rate_limited",
                        "GitHub временно ограничил запросы, повторите позже",
                    ),
                    CollectError::Fetch(_) => ("github_unavailable", "GitHub временно недоступен"),
                };
                let mut job = job.lock().await;
                job.state = JobState::Failed;
                job.stage = "Ошибка".into();
                job.finished = Some(Instant::now());
                job.error = Some((code, message));
                return;
            }
        }
    }
}

#[derive(Debug)]
enum ServiceError {
    InvalidLogin,
    UnknownAccount,
    NotConfigured,
    UnknownJob,
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::InvalidLogin => (
                StatusCode::BAD_REQUEST,
                "github_login_invalid",
                "Укажите имя пользователя GitHub или ссылку на профиль",
            ),
            Self::UnknownAccount => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "github_account_not_found",
                "Пользователь GitHub не найден",
            ),
            Self::NotConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "github_not_configured",
                "Анализ GitHub не настроен",
            ),
            Self::UnknownJob => (
                StatusCode::NOT_FOUND,
                "analysis_not_found",
                "Анализ не найден",
            ),
        };
        (status, Json(json!({ "code": code, "message": message }))).into_response()
    }
}
