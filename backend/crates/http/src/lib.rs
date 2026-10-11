//! Inbound HTTP adapter for the application use cases.

mod error;
mod extract;
mod pdf;

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, FromRequest, Request, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE, COOKIE, ORIGIN, SET_COOKIE,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{any, delete, get, patch, post, put};
use axum::{Json, Router};
use chrono::Utc;
use fsp_application::{
    ApplicationError, CandidateSearch, CompanyPatch, EmailChallengeInput, EmailVerifyInput,
    PlatformService, ProfilePatch, SessionView,
};
use fsp_domain::{Grade, Specialization};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

pub use error::{ApiError, Detail};
pub use extract::IdempotencyKey;
use extract::{ContractJson, ContractPath, ContractQuery, reject_duplicate_query};

const REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
const IDEMPOTENT_REPLAY: HeaderName = HeaderName::from_static("x-idempotent-replay");
const SESSION_COOKIE: &str = "fsp_session";
/// JSON body ceiling, shared with the mock's `MAX_JSON`.
const MAX_JSON_BODY: usize = 512 * 1024;
/// Profile-import envelope ceiling: the mock accepts a 2 MiB `file` part,
/// the extra room covers the multipart framing.
const MAX_IMPORT_BODY: usize = 3 * 1024 * 1024;
/// The mock's `MAX_PDF`.
const MAX_PDF_FILE: usize = 2 * 1024 * 1024;
/// Allow the integrations' 45s total budget plus storage/envelope overhead,
/// while finishing before nginx's 60s proxy read deadline.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(55);

#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub allowed_origins: Vec<String>,
    pub secure_cookie: bool,
    pub public_web_url: String,
    pub github_callback_url: String,
    pub static_dir: Option<PathBuf>,
}

#[derive(Clone)]
struct AppState {
    service: Arc<PlatformService>,
    config: Arc<HttpConfig>,
}

pub fn router(config: HttpConfig, service: Arc<PlatformService>) -> Router {
    let state = AppState {
        service,
        config: Arc::new(config),
    };
    let static_dir = state.config.static_dir.clone();
    // Routes without a request body reject one with `400 body_not_allowed`, and
    // every routed request rejects repeated query keys with
    // `duplicate_query_parameter`; both guards sit on routes so unmatched paths
    // still answer `404 not_found` exactly like the mock.
    let bodyless = Router::new()
        .route("/health", get(health))
        .route("/api/v1/meta", get(meta))
        .route("/api/v1/auth/session", delete(logout))
        .route("/api/v1/session", get(session))
        .route("/api/v1/me/profile", get(get_profile))
        .route("/api/v1/company", get(get_company))
        .route("/api/v1/needs", get(get_needs))
        .route("/api/v1/needs/{need_id}", get(get_need))
        .route("/api/v1/needs/{need_id}", delete(delete_need))
        .route("/api/v1/needs/{need_id}/publish", post(publish_need))
        .route("/api/v1/needs/{need_id}/unpublish", post(unpublish_need))
        .route("/api/v1/needs/{need_id}/graph", get(need_graph))
        .route("/api/v1/vacancies", get(search_vacancies))
        .route("/api/v1/vacancies/{vacancy_id}", get(get_vacancy))
        .route("/api/v1/candidates", get(search_candidates))
        .route("/api/v1/candidates/{candidate_id}", get(get_candidate))
        .route(
            "/api/v1/integrations/github/authorize",
            get(authorize_github),
        )
        .route("/api/v1/integrations/github/callback", get(github_callback))
        .route("/api/v1/me/sources", get(get_sources))
        .route("/api/v1/me/skills", get(get_skills))
        .route("/api/v1/me/graph", get(get_my_graph))
        .route(
            "/api/v1/candidates/{candidate_id}/graph",
            get(get_candidate_graph),
        )
        .route("/api/v1/me/sources/{source_id}/sync", post(sync_source))
        .route("/api/v1/jobs/{job_id}", get(get_job))
        .route("/api/v1/shortlists", get(get_shortlists))
        .route("/api/v1/shortlists/{shortlist_id}", get(get_shortlist))
        .route(
            "/api/v1/shortlists/{shortlist_id}",
            delete(delete_shortlist),
        )
        .route(
            "/api/v1/shortlists/{shortlist_id}/members/{candidate_id}",
            put(add_shortlist_member),
        )
        .route(
            "/api/v1/shortlists/{shortlist_id}/members/{candidate_id}",
            delete(remove_shortlist_member),
        )
        .route("/api/v1/invitations", get(get_invitations))
        .route("/api/v1/invitations/{invitation_id}", get(get_invitation))
        .route(
            "/api/v1/invitations/{invitation_id}/withdraw",
            post(withdraw_invitation),
        )
        .route("/api/v1/me/invitations", get(get_my_invitations))
        .route(
            "/api/v1/me/invitations/{invitation_id}",
            get(get_my_invitation),
        )
        .route(
            "/api/v1/me/invitations/{invitation_id}/view",
            post(view_my_invitation),
        )
        .route(
            "/api/v1/me/invitations/{invitation_id}/decline",
            post(decline_my_invitation),
        )
        .route(
            "/api/v1/candidates/{candidate_id}/disclosure",
            get(get_candidate_disclosure),
        )
        .route("/api/v1/applications", get(list_applications))
        .route("/api/v1/analytics/hiring-flow", get(hiring_flow_analytics))
        .route(
            "/api/v1/applications/{application_id}",
            get(get_application),
        )
        .route(
            "/api/v1/applications/{application_id}/view",
            post(view_application),
        )
        .route(
            "/api/v1/applications/{application_id}/accept",
            post(accept_application),
        )
        .route(
            "/api/v1/applications/{application_id}/decline",
            post(decline_application),
        )
        .route("/api/v1/me/applications", get(get_my_applications))
        .route(
            "/api/v1/me/applications/{application_id}",
            get(get_my_application),
        )
        .route(
            "/api/v1/me/applications/{application_id}/withdraw",
            post(withdraw_my_application),
        )
        .route("/api/v1/me/tasks", get(get_my_tasks))
        .route("/api/v1/tasks", get(list_tasks))
        .route("/api/v1/tasks/{task_id}/archive", post(archive_task))
        .route("/api/v1/task-responses", get(list_task_responses))
        .route("/api/v1/me/gap-plan", get(get_my_gap_plan))
        .route(
            "/api/v1/candidates/{candidate_id}/gap-plan",
            get(get_candidate_gap_plan),
        )
        .route("/api/v1/me/profile/export", get(export_profile))
        .route("/api/v1/me/profile/estimate", post(refresh_estimate))
        .route("/api/v1/me/assessments", get(get_my_assessments))
        .route("/api/v1/me/interviews", get(get_my_interviews))
        .route("/api/v1/me/interviews/{interview_id}", get(get_interview))
        .route(
            "/api/v1/me/assessments/{assessment_id}",
            get(get_assessment),
        )
        .route(
            "/api/v1/me/assessments/{assessment_id}/checks",
            post(run_assessment_checks),
        )
        .route(
            "/api/v1/me/assessments/{assessment_id}/submit",
            post(submit_assessment),
        )
        .route_layer(middleware::from_fn(no_body));
    let bodied = Router::new()
        .route(
            "/api/v1/auth/email/challenges",
            post(create_email_challenge),
        )
        .route("/api/v1/auth/email/verify", post(verify_email))
        .route("/api/v1/me/profile", patch(update_profile))
        .route("/api/v1/company", patch(update_company))
        .route("/api/v1/needs", post(create_need))
        .route("/api/v1/needs/{need_id}", patch(update_need))
        .route("/api/v1/shortlists", post(create_shortlist))
        .route("/api/v1/shortlists/{shortlist_id}", patch(update_shortlist))
        .route("/api/v1/invitations", post(create_invitation))
        .route(
            "/api/v1/me/invitations/{invitation_id}/accept",
            post(accept_my_invitation),
        )
        .route("/api/v1/me/applications", post(create_application))
        .route("/api/v1/tasks", post(create_task))
        .route(
            "/api/v1/me/tasks/{task_id}/responses",
            post(respond_to_task),
        )
        .route(
            "/api/v1/task-responses/{response_id}/review",
            post(review_task_response),
        )
        .route(
            "/api/v1/me/sources/{provider}/connect",
            post(connect_source),
        )
        .route("/api/v1/me/assessments", post(start_assessment))
        .route("/api/v1/me/interviews", post(start_interview))
        .route(
            "/api/v1/me/interviews/{interview_id}/answers",
            post(answer_interview),
        )
        .route(
            "/api/v1/me/interviews/{interview_id}/speech",
            post(synthesize_interview_speech),
        )
        .route(
            "/api/v1/me/assessments/{assessment_id}/answer",
            put(save_assessment_answer),
        )
        .route(
            "/api/v1/me/assessments/{assessment_id}/assistant",
            post(ask_assessment_assistant),
        )
        .route(
            "/api/v1/me/profile/imports",
            post(import_profile).layer(DefaultBodyLimit::max(MAX_IMPORT_BODY)),
        )
        .route(
            "/api/v1/me/profile/imports/{job_id}/confirm",
            post(confirm_profile_import),
        );
    let router: Router = bodyless
        .merge(bodied)
        .route_layer(middleware::from_fn(query_guard))
        .route("/api/{*path}", any(not_found))
        .route("/__mock__", any(not_found))
        .route("/__mock__/{*path}", any(not_found))
        .method_not_allowed_fallback(not_found)
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY))
        .with_state(state.clone());
    // The static fallback and the transport envelope wrap every path, so apply
    // them last, to the whole router.
    let router = if let Some(static_dir) = static_dir {
        router.fallback_service(
            ServeDir::new(&static_dir).fallback(ServeFile::new(static_dir.join("index.html"))),
        )
    } else {
        router
    };
    router
        // Outermost-last ordering: transport renders the envelope for the
        // `ApiError` extensions produced by the timeout and panic guards.
        .layer(CatchPanicLayer::custom(panic_response))
        .layer(middleware::from_fn(with_timeout))
        .layer(middleware::from_fn_with_state(state, transport))
}

fn panic_response(_: Box<dyn std::any::Any + Send>) -> Response {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "Сервис временно недоступен",
    )
    .into_response()
}

async fn with_timeout(request: Request, next: Next) -> Response {
    match tokio::time::timeout(REQUEST_TIMEOUT, next.run(request)).await {
        Ok(response) => response,
        Err(_) => ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Сервис временно недоступен",
        )
        .into_response(),
    }
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

async fn health(State(state): State<AppState>) -> Result<Json<Health>, ApiError> {
    state.service.healthy().await.map_err(|_| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "internal_error",
            "Сервис временно недоступен",
        )
    })?;
    Ok(Json(Health {
        status: "ok",
        service: "fsp-career-api",
        version: env!("CARGO_PKG_VERSION"),
    }))
}

/// Emits an idempotent command's outcome: stored bodies replay verbatim with
/// `X-Idempotent-Replay: true`, fresh results serialize normally.
pub fn idempotent_response<T: Serialize>(
    status: StatusCode,
    outcome: fsp_application::Idempotent<T>,
) -> Response {
    match outcome {
        fsp_application::Idempotent::Replayed { status, body } => {
            let mut response = Response::new(Body::from(body));
            *response.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            response
                .headers_mut()
                .insert(IDEMPOTENT_REPLAY, HeaderValue::from_static("true"));
            response
        }
        fsp_application::Idempotent::Fresh(value) => (status, Json(value)).into_response(),
    }
}

async fn meta(State(state): State<AppState>) -> Json<Value> {
    let catalog = fsp_domain::catalog::catalog();
    let scopes = fsp_domain::knowledge_scopes()
        .into_iter()
        .map(|(specialization, grades)| {
            json!({ "specialization": specialization, "grades": grades })
        })
        .collect::<Vec<_>>();
    let skills = catalog
        .skills
        .iter()
        .map(|item| json!({ "id": item.id, "label": item.label, "category": item.category }))
        .collect::<Vec<_>>();
    let specializations = catalog
        .specializations
        .iter()
        .map(|item| {
            json!({ "id": item.id, "label": item.label, "short": item.short, "group": item.group })
        })
        .collect::<Vec<_>>();
    let grades = catalog
        .grades
        .iter()
        .map(|item| json!({ "id": item.id, "label": item.label, "description": item.description }))
        .collect::<Vec<_>>();
    let categories = catalog
        .skill_categories
        .iter()
        .map(|item| json!({ "id": item.id, "label": item.label }))
        .collect::<Vec<_>>();
    Json(json!({
        "contractVersion": "0.1.0",
        "mode": "live",
        "serverTime": Utc::now().to_rfc3339(),
        "scenario": "live",
        "capabilities": {
            "emailVerification": true, "realEmail": state.service.real_email_configured(), "profilePdf": true, "profilePdfImport": state.service.pdf_import_configured(), "githubFixture": false,
            "fspFixture": true, "matching": true, "directInvitations": true,
            "vacancyPublication": true, "selfApplication": true, "chat": false,
            "recurringTasks": true, "realCodeExecution": false, "realLlm": state.service.llm_configured(),
            "githubOAuth": state.service.github_configured(),
            "githubInsights": state.service.github_insights_configured(),
            "graphInterviews": state.service.interviews_configured(),
            "neuralTts": state.service.speech_configured(),
            "hiringAnalytics": true
        },
        "assessmentPolicy": {
            "successfulGradeChangeCooldownDays": state.service.grade_cooldown_days(), "failedAttemptCooldownDays": 0,
            "serviceFailureCooldownDays": 0, "sameGradeRetryCooldownDays": 0,
            "aiAllowed": true, "externalAiAllowed": true,
            "supportedScopes": scopes
        },
        "competitions": [
            {"id":"competition-demo-api","name":"Демо: проектирование надёжного API","synthetic":true,"skillIds":["rust","postgresql"],"discipline":"Продуктовое программирование"},
            {"id":"competition-demo-systems","name":"Демо: распределённые системы","synthetic":true,"skillIds":["docker","redis"],"discipline":"Продуктовое программирование"}
        ],
        "catalogVersion": catalog.version,
        "grades": grades,
        "specializations": specializations,
        "skillCategories": categories,
        "skills": skills
    }))
}

async fn create_email_challenge(
    State(state): State<AppState>,
    ContractJson(input): ContractJson<EmailChallengeInput>,
) -> Result<(StatusCode, Json<fsp_application::EmailChallengeView>), ApiError> {
    state
        .service
        .request_challenge(input)
        .await
        .map(|view| (StatusCode::CREATED, Json(view)))
        .map_err(api_error)
}

async fn verify_email(
    State(state): State<AppState>,
    ContractJson(input): ContractJson<EmailVerifyInput>,
) -> Result<Response, ApiError> {
    let (session, token) = state.service.verify_email(input).await.map_err(api_error)?;
    let mut response = Json(session).into_response();
    let secure = if state.config.secure_cookie {
        "; Secure"
    } else {
        ""
    };
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000{secure}"
        ))
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Сервис временно недоступен",
            )
        })?,
    );
    Ok(response)
}

async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionView>, ApiError> {
    let token = session_token(&headers)?;
    state
        .service
        .session(token)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    if let Ok(token) = session_token(&headers) {
        state.service.logout(token).await.map_err(api_error)?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_static("fsp_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"),
    );
    Ok(response)
}

async fn get_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::ProfileView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .my_profile(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EstimateQuery {
    answers: Option<EstimateAnswersMode>,
}

/// `keep` answers only the fields the candidate has not edited; `replace` rebuilds them all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EstimateAnswersMode {
    Keep,
    Replace,
}

async fn refresh_estimate(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractQuery(query): ContractQuery<EstimateQuery>,
) -> Result<Json<fsp_application::ProfileView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .refresh_estimate(
            &session,
            query.answers == Some(EstimateAnswersMode::Replace),
        )
        .await
        .map(Json)
        .map_err(api_error)
}

async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractJson(patch): ContractJson<ProfilePatch>,
) -> Result<Json<fsp_application::ProfileView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .update_profile(&session, patch)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_company(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::CompanyView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .company(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn update_company(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractJson(patch): ContractJson<CompanyPatch>,
) -> Result<Json<fsp_application::CompanyView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .update_company(&session, patch)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_needs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::NeedsView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .needs(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn create_need(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractJson(input): ContractJson<fsp_application::NeedInput>,
) -> Result<(StatusCode, Json<fsp_application::NeedView>), ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .create_need(&session, input)
        .await
        .map(|need| (StatusCode::CREATED, Json(need)))
        .map_err(api_error)
}

async fn get_need(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(need_id): ContractPath<String>,
) -> Result<Json<fsp_application::NeedView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .get_need(&session, &need_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn update_need(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(need_id): ContractPath<String>,
    ContractJson(patch): ContractJson<fsp_application::NeedPatch>,
) -> Result<Json<fsp_application::NeedView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .update_need(&session, &need_id, patch)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn delete_need(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(need_id): ContractPath<String>,
) -> Result<StatusCode, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .delete_need(&session, &need_id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(api_error)
}

async fn publish_need(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(need_id): ContractPath<String>,
) -> Result<Json<fsp_application::NeedView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .publish_need(&session, &need_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn unpublish_need(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(need_id): ContractPath<String>,
) -> Result<Json<fsp_application::NeedView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .unpublish_need(&session, &need_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn need_graph(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(need_id): ContractPath<String>,
) -> Result<Json<fsp_application::GraphView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .need_graph(&session, &need_id)
        .await
        .map(Json)
        .map_err(api_error)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VacancyQuery {
    specialization: Option<Specialization>,
    grade: Option<Grade>,
    work_mode: Option<String>,
    skill: Option<String>,
    page: Option<usize>,
    page_size: Option<usize>,
}

async fn search_vacancies(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractQuery(query): ContractQuery<VacancyQuery>,
) -> Result<Json<fsp_application::VacancyPageView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(10);
    if page == 0 || page_size == 0 || page_size > 50 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Проверьте параметры страницы",
        ));
    }
    if query
        .work_mode
        .as_deref()
        .is_some_and(|value| !["remote", "hybrid", "office"].contains(&value))
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Запрос не соответствует контракту",
        ));
    }
    state
        .service
        .search_vacancies(
            &session,
            &fsp_application::VacancySearch {
                specialization: query.specialization,
                grade: query.grade,
                work_mode: query.work_mode,
                skill: query.skill,
                page,
                page_size,
            },
        )
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_vacancy(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(vacancy_id): ContractPath<String>,
) -> Result<Json<fsp_application::VacancyView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .get_vacancy(&session, &vacancy_id)
        .await
        .map(Json)
        .map_err(api_error)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CandidateQuery {
    need_id: Option<String>,
    mode: Option<String>,
    specialization: Option<Specialization>,
    grade: Option<Grade>,
    category_source: Option<fsp_domain::CategorySource>,
    skill: Option<String>,
    fsp: Option<String>,
    page: Option<usize>,
    page_size: Option<usize>,
}

async fn search_candidates(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractQuery(query): ContractQuery<CandidateQuery>,
) -> Result<Json<fsp_application::CandidatePageView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(10);
    if page == 0 || page_size == 0 || page_size > 50 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Проверьте параметры страницы",
        ));
    }
    if query
        .mode
        .as_deref()
        .is_some_and(|value| value != "bank" && value != "recommended")
        || query
            .fsp
            .as_deref()
            .is_some_and(|value| value != "any" && value != "verified")
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Проверьте параметры поиска",
        ));
    }
    state
        .service
        .candidate_search(
            &session,
            CandidateSearch {
                need_id: query.need_id,
                recommended: query.mode.as_deref() == Some("recommended"),
                specialization: query.specialization,
                grade: query.grade,
                category_source: query.category_source,
                skill: query.skill,
                fsp_verified: query.fsp.as_deref() == Some("verified"),
                page,
                page_size,
            },
        )
        .await
        .map(Json)
        .map_err(api_error)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CandidateDetailQuery {
    need_id: Option<String>,
}

async fn get_candidate(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(candidate_id): ContractPath<String>,
    ContractQuery(query): ContractQuery<CandidateDetailQuery>,
) -> Result<Json<fsp_application::AnonymousCandidateView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .candidate(&session, &candidate_id, query.need_id.as_deref())
        .await
        .map(Json)
        .map_err(api_error)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizationUrlView {
    authorization_url: String,
}

async fn authorize_github(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthorizationUrlView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .github_authorization_url(&session, &state.config.github_callback_url)
        .await
        .map(|authorization_url| Json(AuthorizationUrlView { authorization_url }))
        .map_err(api_error)
}

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: String,
    state: String,
}

async fn github_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractQuery(query): ContractQuery<OAuthCallbackQuery>,
) -> Result<Redirect, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .complete_github_authorization(&session, &query.state, &query.code)
        .await
        .map_err(api_error)?;
    Ok(Redirect::to(&format!(
        "{}/candidate/sources?github=authorized",
        state.config.public_web_url.trim_end_matches('/')
    )))
}

async fn get_sources(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::SourcesView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .sources(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_skills(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::SkillsView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .skills(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_my_graph(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::GraphView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .my_graph(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_candidate_graph(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(candidate_id): ContractPath<String>,
) -> Result<Json<fsp_application::GraphView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .candidate_graph(&session, &candidate_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn connect_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(provider): ContractPath<String>,
    ContractJson(input): ContractJson<fsp_application::SourceConnectInput>,
) -> Result<Json<fsp_application::SourceView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .connect_source(&session, &provider, input)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn sync_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(source_id): ContractPath<String>,
) -> Result<(StatusCode, Json<fsp_application::JobView>), ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .sync_source(&session, &source_id)
        .await
        .map(|job| (StatusCode::ACCEPTED, Json(job)))
        .map_err(api_error)
}

async fn get_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(job_id): ContractPath<String>,
) -> Result<Json<fsp_application::JobView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .job(&session, &job_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_shortlists(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::ShortlistsView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .shortlists(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn create_shortlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractJson(input): ContractJson<fsp_application::ShortlistInput>,
) -> Result<(StatusCode, Json<fsp_application::ShortlistView>), ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .create_shortlist(&session, input)
        .await
        .map(|shortlist| (StatusCode::CREATED, Json(shortlist)))
        .map_err(api_error)
}

async fn get_shortlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(shortlist_id): ContractPath<String>,
) -> Result<Json<fsp_application::ShortlistView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .get_shortlist(&session, &shortlist_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn update_shortlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(shortlist_id): ContractPath<String>,
    ContractJson(input): ContractJson<fsp_application::ShortlistInput>,
) -> Result<Json<fsp_application::ShortlistView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .update_shortlist(&session, &shortlist_id, input)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn delete_shortlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(shortlist_id): ContractPath<String>,
) -> Result<StatusCode, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .delete_shortlist(&session, &shortlist_id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(api_error)
}

async fn add_shortlist_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath((shortlist_id, candidate_id)): ContractPath<(String, String)>,
) -> Result<Json<fsp_application::ShortlistView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .add_shortlist_member(&session, &shortlist_id, &candidate_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn remove_shortlist_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath((shortlist_id, candidate_id)): ContractPath<(String, String)>,
) -> Result<Json<fsp_application::ShortlistView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    state
        .service
        .remove_shortlist_member(&session, &shortlist_id, &candidate_id)
        .await
        .map(Json)
        .map_err(api_error)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplicationsQuery {
    need_id: Option<String>,
}

async fn list_applications(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractQuery(query): ContractQuery<ApplicationsQuery>,
) -> Result<Json<fsp_application::ApplicationsView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .company_applications(&session, query.need_id.as_deref())
        .await
        .map(Json)
        .map_err(api_error)
}

async fn hiring_flow_analytics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::HiringFlowAnalyticsView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .hiring_flow_analytics(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(application_id): ContractPath<String>,
) -> Result<Json<fsp_application::ApplicationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .get_application(&session, &application_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn view_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(application_id): ContractPath<String>,
) -> Result<Json<fsp_application::ApplicationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .view_application(&session, &application_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn accept_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(application_id): ContractPath<String>,
) -> Result<Json<fsp_application::ApplicationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .accept_application(&session, &application_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn decline_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(application_id): ContractPath<String>,
) -> Result<Json<fsp_application::ApplicationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .decline_application(&session, &application_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_my_applications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::ApplicationsView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .my_applications(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn create_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    IdempotencyKey(key): IdempotencyKey,
    ContractJson(input): ContractJson<fsp_application::ApplicationInput>,
) -> Result<Response, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    let body = serde_json::to_value(&input).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Запрос не соответствует контракту",
        )
    })?;
    let command_session = &session;
    let outcome = state
        .service
        .idempotent(
            &session,
            fsp_application::IdempotencyContext {
                operation: "createApplication",
                path: "/api/v1/me/applications",
                key: &key,
                body: &body,
                status: StatusCode::CREATED.as_u16(),
            },
            |commands| async move { commands.create_application(command_session, input).await },
        )
        .await
        .map_err(api_error)?;
    Ok(idempotent_response(StatusCode::CREATED, outcome))
}

async fn get_my_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(application_id): ContractPath<String>,
) -> Result<Json<fsp_application::ApplicationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .get_my_application(&session, &application_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn withdraw_my_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(application_id): ContractPath<String>,
) -> Result<Json<fsp_application::ApplicationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .withdraw_my_application(&session, &application_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_my_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::MyTasksView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .my_tasks(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn respond_to_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    IdempotencyKey(key): IdempotencyKey,
    ContractPath(task_id): ContractPath<String>,
    ContractJson(input): ContractJson<fsp_application::TaskResponseInput>,
) -> Result<Response, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    let body = serde_json::to_value(&input).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Запрос не соответствует контракту",
        )
    })?;
    let path = format!("/api/v1/me/tasks/{task_id}/responses");
    let command_session = &session;
    let outcome = state
        .service
        .idempotent(
            &session,
            fsp_application::IdempotencyContext {
                operation: "respondToTask",
                path: &path,
                key: &key,
                body: &body,
                status: StatusCode::CREATED.as_u16(),
            },
            |commands| async move {
                commands
                    .respond_to_task(command_session, &task_id, input)
                    .await
            },
        )
        .await
        .map_err(api_error)?;
    Ok(idempotent_response(StatusCode::CREATED, outcome))
}

async fn get_my_assessments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::AssessmentsView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .my_assessments(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_my_interviews(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::InterviewsView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .my_interviews(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn start_interview(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractJson(input): ContractJson<fsp_application::InterviewStartInput>,
) -> Result<(StatusCode, Json<fsp_application::InterviewView>), ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .start_interview(&session, input)
        .await
        .map(|interview| (StatusCode::CREATED, Json(interview)))
        .map_err(api_error)
}

async fn get_interview(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(interview_id): ContractPath<String>,
) -> Result<Json<fsp_application::InterviewView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .get_interview(&session, &interview_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn answer_interview(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(interview_id): ContractPath<String>,
    ContractJson(input): ContractJson<fsp_application::InterviewAnswerInput>,
) -> Result<Json<fsp_application::InterviewView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .answer_interview(&session, &interview_id, input)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn synthesize_interview_speech(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(interview_id): ContractPath<String>,
    ContractJson(input): ContractJson<fsp_application::InterviewSpeechInput>,
) -> Result<Json<fsp_application::SpeechAudioView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .synthesize_interview_question(&session, &interview_id, input)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn start_assessment(
    State(state): State<AppState>,
    headers: HeaderMap,
    IdempotencyKey(key): IdempotencyKey,
    ContractJson(input): ContractJson<fsp_application::AssessmentStartInput>,
) -> Result<Response, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    let body = serde_json::to_value(&input).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Запрос не соответствует контракту",
        )
    })?;
    let command_session = &session;
    let outcome = state
        .service
        .idempotent(
            &session,
            fsp_application::IdempotencyContext {
                operation: "startAssessment",
                path: "/api/v1/me/assessments",
                key: &key,
                body: &body,
                status: StatusCode::CREATED.as_u16(),
            },
            |commands| async move { commands.start_assessment(command_session, input).await },
        )
        .await
        .map_err(api_error)?;
    Ok(idempotent_response(StatusCode::CREATED, outcome))
}

async fn get_assessment(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(assessment_id): ContractPath<String>,
) -> Result<Json<fsp_application::AssessmentView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .get_assessment(&session, &assessment_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn save_assessment_answer(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(assessment_id): ContractPath<String>,
    ContractJson(input): ContractJson<fsp_domain::AssessmentAnswer>,
) -> Result<Json<fsp_application::AssessmentView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .save_assessment_answer(&session, &assessment_id, input)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn run_assessment_checks(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(assessment_id): ContractPath<String>,
) -> Result<Response, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .run_assessment_checks(&session, &assessment_id)
        .await
        .map(|view| (StatusCode::ACCEPTED, Json(view)).into_response())
        .map_err(api_error)
}

async fn ask_assessment_assistant(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(assessment_id): ContractPath<String>,
    ContractJson(input): ContractJson<fsp_application::AssistantInput>,
) -> Result<Json<fsp_application::AssistantReplyView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .ask_assessment_assistant(&session, &assessment_id, input)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn submit_assessment(
    State(state): State<AppState>,
    headers: HeaderMap,
    IdempotencyKey(key): IdempotencyKey,
    ContractPath(assessment_id): ContractPath<String>,
) -> Result<Response, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    let path = format!("/api/v1/me/assessments/{assessment_id}/submit");
    let command_session = &session;
    let outcome = state
        .service
        .idempotent(
            &session,
            fsp_application::IdempotencyContext {
                operation: "submitAssessment",
                path: &path,
                key: &key,
                body: &serde_json::Value::Null,
                status: StatusCode::ACCEPTED.as_u16(),
            },
            |commands| async move {
                commands
                    .submit_assessment(command_session, &assessment_id)
                    .await
            },
        )
        .await
        .map_err(api_error)?;
    Ok(idempotent_response(StatusCode::ACCEPTED, outcome))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GapPlanQuery {
    need_id: Option<String>,
}

async fn get_my_gap_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractQuery(query): ContractQuery<GapPlanQuery>,
) -> Result<Json<fsp_application::GapPlanView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .my_gap_plan(&session, query.need_id.as_deref())
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_candidate_gap_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(candidate_id): ContractPath<String>,
    ContractQuery(query): ContractQuery<GapPlanQuery>,
) -> Result<Json<fsp_application::GapPlanView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .candidate_gap_plan(&session, &candidate_id, query.need_id.as_deref())
        .await
        .map(Json)
        .map_err(api_error)
}

/// The mock's `parseMultipart`: a single `file` part whose declared type is
/// `application/pdf`, 1 byte to 2 MiB, with a `%PDF-` header and `%%EOF`
/// marker inside the trailing 2048 bytes.
async fn import_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !content_type.starts_with("multipart/form-data;") {
        return Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "multipart_required",
            "Ожидается multipart/form-data",
        ));
    }
    let invalid_multipart = || {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_multipart",
            "Не удалось прочитать форму загрузки",
        )
    };
    let mut multipart = axum::extract::Multipart::from_request(request, &state)
        .await
        .map_err(|_| invalid_multipart())?;
    let mut parts = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| invalid_multipart())?
    {
        let name = field.name().unwrap_or_default().to_owned();
        let declared_type = field.content_type().map(str::to_owned);
        let bytes = field.bytes().await.map_err(|_| invalid_multipart())?;
        parts.push((name, declared_type, bytes));
    }
    let [(name, declared_type, bytes)] = <[_; 1]>::try_from(parts).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "file_required",
            "Передайте один PDF в поле file",
        )
    })?;
    if name != "file" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "file_required",
            "Передайте один PDF в поле file",
        ));
    }
    if declared_type.as_deref() != Some("application/pdf") {
        return Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "pdf_required",
            "Поддерживаются только PDF",
        ));
    }
    if bytes.is_empty() || bytes.len() > MAX_PDF_FILE {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_too_large",
            "Размер PDF должен быть от 1 байта до 2 МиБ",
        ));
    }
    let tail = &bytes[bytes.len().saturating_sub(2048)..];
    if !bytes.starts_with(b"%PDF-") || !tail.windows(5).any(|window| window == b"%%EOF") {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_pdf",
            "Файл не содержит ожидаемую сигнатуру PDF",
        ));
    }
    state
        .service
        .import_profile(&session, &bytes)
        .await
        .map(|view| (StatusCode::ACCEPTED, Json(view)).into_response())
        .map_err(api_error)
}

async fn confirm_profile_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(job_id): ContractPath<String>,
    ContractJson(input): ContractJson<fsp_application::ImportConfirmInput>,
) -> Result<Json<fsp_application::ProfileView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .confirm_profile_import(&session, &job_id, input)
        .await
        .map(Json)
        .map_err(api_error)
}

/// The mock's `exportMyProfile`: a generated PDF download, not JSON.
async fn export_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    let profile = state
        .service
        .export_profile(&session)
        .await
        .map_err(api_error)?;
    let bytes = pdf::profile_pdf(&profile).map_err(|error| {
        tracing::error!(error = %error, "profile PDF render failed");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Сервис временно недоступен",
        )
    })?;
    Ok((
        [
            (CONTENT_TYPE, "application/pdf"),
            (
                CONTENT_DISPOSITION,
                "attachment; filename=\"fsp-profile.pdf\"",
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn list_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::TasksView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .tasks(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn create_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractJson(input): ContractJson<fsp_application::TaskInput>,
) -> Result<(StatusCode, Json<fsp_application::TaskView>), ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .create_task(&session, input)
        .await
        .map(|task| (StatusCode::CREATED, Json(task)))
        .map_err(api_error)
}

async fn archive_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(task_id): ContractPath<String>,
) -> Result<Json<fsp_application::TaskView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .archive_task(&session, &task_id)
        .await
        .map(Json)
        .map_err(api_error)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskResponsesQuery {
    task_id: Option<String>,
}

async fn list_task_responses(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractQuery(query): ContractQuery<TaskResponsesQuery>,
) -> Result<Json<fsp_application::TaskResponsesView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .task_responses(&session, query.task_id.as_deref())
        .await
        .map(Json)
        .map_err(api_error)
}

async fn review_task_response(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(response_id): ContractPath<String>,
    ContractJson(input): ContractJson<fsp_application::TaskReviewInput>,
) -> Result<Json<fsp_application::TaskResponseView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .review_task_response(&session, &response_id, input)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_invitations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::InvitationsView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .invitations(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn create_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    IdempotencyKey(key): IdempotencyKey,
    ContractJson(input): ContractJson<fsp_application::InvitationInput>,
) -> Result<Response, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    let body = serde_json::to_value(&input).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Запрос не соответствует контракту",
        )
    })?;
    let command_session = &session;
    let outcome = state
        .service
        .idempotent(
            &session,
            fsp_application::IdempotencyContext {
                operation: "createInvitation",
                path: "/api/v1/invitations",
                key: &key,
                body: &body,
                status: StatusCode::CREATED.as_u16(),
            },
            |commands| async move { commands.create_invitation(command_session, input).await },
        )
        .await
        .map_err(api_error)?;
    Ok(idempotent_response(StatusCode::CREATED, outcome))
}

async fn get_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(invitation_id): ContractPath<String>,
) -> Result<Json<fsp_application::InvitationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .get_invitation(&session, &invitation_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn withdraw_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(invitation_id): ContractPath<String>,
) -> Result<Json<fsp_application::InvitationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .withdraw_invitation(&session, &invitation_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_my_invitations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<fsp_application::InvitationsView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .my_invitations(&session)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_my_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(invitation_id): ContractPath<String>,
) -> Result<Json<fsp_application::InvitationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .get_invitation(&session, &invitation_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn view_my_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(invitation_id): ContractPath<String>,
) -> Result<Json<fsp_application::InvitationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .view_my_invitation(&session, &invitation_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn accept_my_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(invitation_id): ContractPath<String>,
    ContractJson(input): ContractJson<fsp_application::AcceptInvitationInput>,
) -> Result<Json<fsp_application::InvitationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .accept_my_invitation(&session, &invitation_id, input)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn decline_my_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(invitation_id): ContractPath<String>,
) -> Result<Json<fsp_application::InvitationView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "candidate")?;
    state
        .service
        .decline_my_invitation(&session, &invitation_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_candidate_disclosure(
    State(state): State<AppState>,
    headers: HeaderMap,
    ContractPath(candidate_id): ContractPath<String>,
) -> Result<Json<fsp_application::DisclosureView>, ApiError> {
    let session = authenticated(&state, &headers).await?;
    require_role(&session, "employer")?;
    state
        .service
        .candidate_disclosure(&session, &candidate_id)
        .await
        .map(Json)
        .map_err(api_error)
}

fn require_role(session: &SessionView, role: &str) -> Result<(), ApiError> {
    let allowed = matches!(
        (session.role, role),
        (fsp_domain::Role::Candidate, "candidate") | (fsp_domain::Role::Employer, "employer")
    );
    if allowed {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "role_forbidden",
            "Действие недоступно для этой роли",
        ))
    }
}

async fn not_found() -> ApiError {
    ApiError::not_found("Маршрут не найден")
}

/// Rejects a body on a route that takes none (`400 body_not_allowed`), or
/// `413 body_too_large` past the JSON ceiling — the mock's order.
async fn no_body(request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();
    match axum::body::to_bytes(body, MAX_JSON_BODY).await {
        Ok(bytes) if bytes.is_empty() => next.run(Request::from_parts(parts, Body::empty())).await,
        Ok(_) => ApiError::new(
            StatusCode::BAD_REQUEST,
            "body_not_allowed",
            "Этот маршрут не принимает тело запроса",
        )
        .into_response(),
        Err(_) => ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "body_too_large",
            "Файл или запрос превышает допустимый размер",
        )
        .into_response(),
    }
}

/// Rejects repeated query keys (`400 duplicate_query_parameter`) on every routed
/// request, like the mock does for all of them.
async fn query_guard(request: Request, next: Next) -> Response {
    match reject_duplicate_query(request.uri().query().unwrap_or_default()) {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

async fn authenticated(state: &AppState, headers: &HeaderMap) -> Result<SessionView, ApiError> {
    state
        .service
        .session(session_token(headers)?)
        .await
        .map_err(api_error)
}

fn session_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    headers
        .get(COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .map(str::trim)
                .find_map(|cookie| cookie.strip_prefix("fsp_session="))
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "Войдите в аккаунт",
            )
        })
}

fn api_error(error: ApplicationError) -> ApiError {
    ApiError::new(
        StatusCode::from_u16(error.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        error.code,
        error.message,
    )
}

async fn transport(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let foreign = request.headers().get(ORIGIN).is_some_and(|origin| {
        !state
            .config
            .allowed_origins
            .iter()
            .any(|allowed| origin.as_bytes() == allowed.as_bytes())
    });
    let mut response = if foreign {
        ApiError::forbidden("origin_not_allowed", "Этот источник запроса не разрешён")
            .into_response()
    } else {
        next.run(request).await
    };
    if let Some(error) = response.extensions_mut().remove::<ApiError>() {
        response = error.render(&request_id);
    }
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        headers.insert(REQUEST_ID, value);
    }
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsp_application::{
        ConsumeChallenge, IdempotencyClaim, PendingChallenge, PlatformRepository, RepositoryError,
        RepositoryTransaction, VerificationDelivery,
    };
    use fsp_domain::{
        Account, CandidateIndexRecord, CandidateProfile, Company, HiringNeedRecord, JobRecord,
        SourceRecord,
    };

    struct EmptyRepository;
    #[async_trait::async_trait]
    impl PlatformRepository for EmptyRepository {
        async fn lock_scope(&self, _: &str) -> Result<(), RepositoryError> {
            unreachable!()
        }
        async fn lock_candidate(&self, _: &str) -> Result<(), RepositoryError> {
            unreachable!()
        }

        async fn transaction(&self) -> Result<Box<dyn RepositoryTransaction>, RepositoryError> {
            Err(RepositoryError::Storage("test stub".into()))
        }

        async fn account_by_email(&self, _: &str) -> Result<Option<Account>, RepositoryError> {
            Ok(None)
        }

        async fn create_account(&self, _: &Account, _: &str) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn create_candidate(&self, _: &CandidateProfile) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn create_company(&self, _: &Company) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn candidate(&self, _: &str) -> Result<Option<CandidateProfile>, RepositoryError> {
            Ok(None)
        }

        async fn save_candidate(&self, _: &CandidateProfile) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn save_candidate_guarded(
            &self,
            _: &CandidateProfile,
            _: u64,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn company(&self, _: &str) -> Result<Option<Company>, RepositoryError> {
            Ok(None)
        }

        async fn save_company(&self, _: &Company) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn candidates(&self) -> Result<Vec<CandidateIndexRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn candidate_index(
            &self,
            _: &str,
        ) -> Result<Option<CandidateIndexRecord>, RepositoryError> {
            Ok(None)
        }

        async fn save_candidate_index(
            &self,
            _: &CandidateIndexRecord,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn issue_challenge(
            &self,
            _: &PendingChallenge,
            _: bool,
        ) -> Result<fsp_application::IssueChallenge, RepositoryError> {
            Ok(fsp_application::IssueChallenge::Locked)
        }

        async fn mark_email_verified(&self, _: &str) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn consume_challenge(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<(ConsumeChallenge, Option<PendingChallenge>), RepositoryError> {
            Ok((ConsumeChallenge::Missing, None))
        }

        async fn create_session(&self, _: &str, _: &str, _: &str) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn account_for_session(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Option<Account>, RepositoryError> {
            Ok(None)
        }

        async fn delete_session(&self, _: &str) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn needs(&self, _: &str) -> Result<Vec<HiringNeedRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn need(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Option<HiringNeedRecord>, RepositoryError> {
            Ok(None)
        }

        async fn save_need(&self, _: &HiringNeedRecord) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn save_need_guarded(
            &self,
            _: &HiringNeedRecord,
            _: u64,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn delete_need(&self, _: &str, _: &str) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn published_needs(&self) -> Result<Vec<HiringNeedRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn shortlists(
            &self,
            _: &str,
        ) -> Result<Vec<fsp_domain::ShortlistRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn shortlist(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Option<fsp_domain::ShortlistRecord>, RepositoryError> {
            Ok(None)
        }

        async fn save_shortlist(
            &self,
            _: &fsp_domain::ShortlistRecord,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn delete_shortlist(&self, _: &str, _: &str) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn invitations(
            &self,
            _: &str,
        ) -> Result<Vec<fsp_domain::InvitationRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn candidate_invitations(
            &self,
            _: &str,
        ) -> Result<Vec<fsp_domain::InvitationRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn invitation(
            &self,
            _: &str,
        ) -> Result<Option<fsp_domain::InvitationRecord>, RepositoryError> {
            Ok(None)
        }

        async fn save_invitation(
            &self,
            _: &fsp_domain::InvitationRecord,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn save_invitation_guarded(
            &self,
            _: &fsp_domain::InvitationRecord,
            _: &[fsp_domain::InteractionStatus],
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn disclosure(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Option<fsp_domain::DisclosureRecord>, RepositoryError> {
            Ok(None)
        }

        async fn save_disclosure(
            &self,
            _: &fsp_domain::DisclosureRecord,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn delete_disclosure(&self, _: &str, _: &str) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn company_applications(
            &self,
            _: &str,
        ) -> Result<Vec<fsp_domain::ApplicationRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn candidate_applications(
            &self,
            _: &str,
        ) -> Result<Vec<fsp_domain::ApplicationRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn application(
            &self,
            _: &str,
        ) -> Result<Option<fsp_domain::ApplicationRecord>, RepositoryError> {
            Ok(None)
        }

        async fn save_application(
            &self,
            _: &fsp_domain::ApplicationRecord,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn save_application_guarded(
            &self,
            _: &fsp_domain::ApplicationRecord,
            _: &[fsp_domain::InteractionStatus],
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn tasks(&self, _: &str) -> Result<Vec<fsp_domain::TaskRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn active_tasks(&self) -> Result<Vec<fsp_domain::TaskRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn task(&self, _: &str) -> Result<Option<fsp_domain::TaskRecord>, RepositoryError> {
            Ok(None)
        }

        async fn save_task(&self, _: &fsp_domain::TaskRecord) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn candidate_task_responses(
            &self,
            _: &str,
        ) -> Result<Vec<fsp_domain::TaskResponseRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn company_task_responses(
            &self,
            _: &str,
        ) -> Result<Vec<fsp_domain::TaskResponseRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn task_response(
            &self,
            _: &str,
        ) -> Result<Option<fsp_domain::TaskResponseRecord>, RepositoryError> {
            Ok(None)
        }

        async fn task_response_for(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Option<fsp_domain::TaskResponseRecord>, RepositoryError> {
            Ok(None)
        }

        async fn save_task_response(
            &self,
            _: &fsp_domain::TaskResponseRecord,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn review_task_response(
            &self,
            _: &fsp_domain::TaskResponseRecord,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn candidate_assessments(
            &self,
            _: &str,
        ) -> Result<Vec<fsp_domain::AssessmentRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn assessment(
            &self,
            _: &str,
        ) -> Result<Option<fsp_domain::AssessmentRecord>, RepositoryError> {
            Ok(None)
        }

        async fn save_assessment(
            &self,
            _: &fsp_domain::AssessmentRecord,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn save_assessment_guarded(
            &self,
            _: &fsp_domain::AssessmentRecord,
            _: u64,
            _: &[fsp_domain::AssessmentStatus],
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn save_oauth_state(&self, _: &str, _: &str, _: &str) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn consume_oauth_state(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<bool, RepositoryError> {
            Ok(false)
        }

        async fn save_github_token(&self, _: &str, _: &str) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn github_token(&self, _: &str) -> Result<Option<String>, RepositoryError> {
            Ok(None)
        }

        async fn sources(&self, _: &str) -> Result<Vec<SourceRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn github_sources(&self, _: &str) -> Result<Vec<SourceRecord>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn save_source(&self, _: &SourceRecord) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn save_job(&self, _: &JobRecord) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn save_job_unconfirmed(&self, _: &JobRecord) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn job(&self, _: &str, _: &str) -> Result<Option<JobRecord>, RepositoryError> {
            Ok(None)
        }

        async fn idempotency_claim(
            &self,
            _: &str,
            _: &str,
        ) -> Result<IdempotencyClaim, RepositoryError> {
            Ok(fsp_application::IdempotencyClaim::InFlight)
        }

        async fn idempotency_finish(
            &self,
            _: &str,
            _: u16,
            _: &str,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn sweep_expired(&self, _: &str) -> Result<u64, RepositoryError> {
            Ok(0)
        }

        async fn ping(&self) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn seed_demo(&self, _: &str) -> Result<(), RepositoryError> {
            Ok(())
        }
    }

    struct Delivery;
    #[async_trait::async_trait]
    impl VerificationDelivery for Delivery {
        fn code_for(&self) -> Result<String, fsp_application::IntegrationError> {
            Ok("123456".into())
        }
        async fn deliver(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<(), fsp_application::IntegrationError> {
            Ok(())
        }
        fn contract_name(&self) -> &'static str {
            "mock_mailbox"
        }
    }

    fn app_with_static(static_dir: Option<PathBuf>) -> Router {
        let service = Arc::new(PlatformService::new(
            Arc::new(EmptyRepository),
            Arc::new(Delivery),
        ));
        router(
            HttpConfig {
                allowed_origins: vec!["http://127.0.0.1:5173".into()],
                secure_cookie: false,
                public_web_url: "http://127.0.0.1:5173".into(),
                github_callback_url: "http://127.0.0.1:8080/api/v1/integrations/github/callback"
                    .into(),
                static_dir,
            },
            service,
        )
    }

    fn app() -> Router {
        app_with_static(None)
    }

    #[tokio::test(start_paused = true)]
    async fn bounded_integrations_finish_before_the_outer_deadline() {
        use axum::body::Body;
        use tower::ServiceExt;

        for status in [StatusCode::OK, StatusCode::BAD_GATEWAY] {
            let app = Router::new()
                .route(
                    "/",
                    get(move || async move {
                        tokio::time::sleep(std::time::Duration::from_secs(45)).await;
                        status
                    }),
                )
                .layer(middleware::from_fn(with_timeout));
            let response = app.oneshot(Request::new(Body::empty())).await.unwrap();
            assert_eq!(response.status(), status);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_handlers_still_return_a_contract_error() {
        use axum::body::Body;
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let app = Router::new()
            .route("/", get(std::future::pending::<StatusCode>))
            .layer(middleware::from_fn(with_timeout));
        let started = tokio::time::Instant::now();
        let response = app.oneshot(Request::new(Body::empty())).await.unwrap();
        assert_eq!(started.elapsed(), std::time::Duration::from_secs(55));
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let envelope = response
            .extensions()
            .get::<ApiError>()
            .unwrap()
            .render("timeout-test");
        let bytes = envelope.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["code"], "internal_error");
        assert_eq!(body["requestId"], "timeout-test");
    }

    #[tokio::test]
    async fn health_and_origin_boundary_are_active() {
        use axum::body::Body;
        use axum::http::Method;
        use tower::ServiceExt;
        let response = app()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = app()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .header(ORIGIN, "https://evil.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn static_frontend_serves_deep_links_without_masking_api_404s() {
        use axum::body::Body;
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let static_dir = std::env::temp_dir().join(format!("fsp-http-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&static_dir).unwrap();
        std::fs::write(
            static_dir.join("index.html"),
            "<!doctype html><title>SPA</title>",
        )
        .unwrap();
        let app = app_with_static(Some(static_dir.clone()));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/candidate/profile")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(body.starts_with(b"<!doctype html>"));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/unknown")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        std::fs::remove_dir_all(static_dir).unwrap();
    }

    #[tokio::test]
    async fn transport_rejections_use_the_contract_envelope() {
        use axum::body::Body;
        use axum::http::Method;
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        async fn code_of(app: Router, request: Request<Body>) -> (StatusCode, Value) {
            let response = app.oneshot(request).await.unwrap();
            let status = response.status();
            assert!(response.headers().contains_key(REQUEST_ID));
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let envelope: Value = serde_json::from_slice(&body).expect("envelope JSON");
            assert!(envelope["requestId"].is_string());
            (status, envelope)
        }
        let json = |body: &str| {
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/auth/email/challenges")
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap()
        };

        let (status, envelope) = code_of(app(), json("{")).await;
        assert_eq!(
            (status, envelope["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("invalid_json"))
        );

        let (status, envelope) = code_of(app(), json("{\"email\": 5}")).await;
        assert_eq!(
            (status, envelope["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("validation_error"))
        );
        assert!(
            envelope["details"]
                .as_array()
                .is_some_and(|d| !d.is_empty())
        );

        let (status, envelope) = code_of(
            app(),
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/auth/email/challenges")
                .header("content-type", "text/plain")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
        assert_eq!(
            (status, envelope["code"].as_str()),
            (StatusCode::UNSUPPORTED_MEDIA_TYPE, Some("json_required"))
        );

        let (status, envelope) = code_of(
            app(),
            Request::builder()
                .uri("/api/v1/candidates?page=1&page=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(
            (status, envelope["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("duplicate_query_parameter"))
        );

        let (status, envelope) = code_of(
            app(),
            Request::builder()
                .uri("/health")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
        assert_eq!(
            (status, envelope["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("body_not_allowed"))
        );

        let (status, envelope) = code_of(
            app(),
            Request::builder()
                .uri("/api/v1/no-such-route?page=1&page=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(
            (status, envelope["code"].as_str()),
            (StatusCode::NOT_FOUND, Some("not_found"))
        );

        let (status, envelope) = code_of(app(), json(&" ".repeat(MAX_JSON_BODY + 1))).await;
        assert_eq!(
            (status, envelope["code"].as_str()),
            (StatusCode::PAYLOAD_TOO_LARGE, Some("body_too_large"))
        );
    }
}
