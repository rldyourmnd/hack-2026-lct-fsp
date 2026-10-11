use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::domain::{BACKEND_MIDDLE, InterviewListView, InterviewView, ResponseMode};
use crate::engine::{InterviewService, ServiceError};

#[derive(Clone)]
pub struct HttpState {
    service: Arc<InterviewService>,
    api_token: Arc<str>,
}

pub fn router(service: Arc<InterviewService>, api_token: String) -> Router {
    let state = HttpState {
        service,
        api_token: api_token.into(),
    };
    Router::new()
        .route("/health", get(health))
        .route("/v1/blueprints/backend-middle-v1", get(get_blueprint))
        .route("/v1/interviews", post(start_interview))
        .route(
            "/v1/candidates/{candidate_id}/interviews",
            get(list_interviews),
        )
        .route("/v1/interviews/{interview_id}", get(get_interview))
        .route(
            "/v1/interviews/{interview_id}/answers",
            post(answer_interview),
        )
        .layer(DefaultBodyLimit::max(32 * 1024))
        .with_state(state)
}

async fn health(State(state): State<HttpState>) -> Result<Json<Health>, ApiError> {
    state.service.healthy().await?;
    Ok(Json(Health { status: "ok" }))
}

async fn list_interviews(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(candidate_id): Path<String>,
) -> Result<Json<InterviewListView>, ApiError> {
    authorize(&state, &headers)?;
    Ok(Json(state.service.list(&candidate_id).await?))
}

async fn get_blueprint(
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Json<&'static crate::domain::InterviewBlueprint>, ApiError> {
    authorize(&state, &headers)?;
    Ok(Json(&BACKEND_MIDDLE))
}

async fn start_interview(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(request): Json<StartInterviewRequest>,
) -> Result<(StatusCode, Json<InterviewView>), ApiError> {
    authorize(&state, &headers)?;
    let max_turns = request.max_turns.unwrap_or(10);
    let interview = state
        .service
        .start(&request.candidate_id, &request.blueprint_id, max_turns)
        .await?;
    Ok((StatusCode::CREATED, Json(interview)))
}

async fn get_interview(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(interview_id): Path<String>,
) -> Result<Json<InterviewView>, ApiError> {
    authorize(&state, &headers)?;
    Ok(Json(state.service.get(&interview_id).await?))
}

async fn answer_interview(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(interview_id): Path<String>,
    Json(request): Json<AnswerInterviewRequest>,
) -> Result<Json<InterviewView>, ApiError> {
    authorize(&state, &headers)?;
    Ok(Json(
        state
            .service
            .answer(
                &interview_id,
                &request.question_id,
                &request.answer,
                request.response_mode,
                request.revision,
            )
            .await?,
    ))
}

fn authorize(state: &HttpState, headers: &HeaderMap) -> Result<(), ApiError> {
    let supplied = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if supplied == Some(state.api_token.as_ref()) {
        Ok(())
    } else {
        Err(ApiError::Unauthorized)
    }
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartInterviewRequest {
    candidate_id: String,
    blueprint_id: String,
    max_turns: Option<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AnswerInterviewRequest {
    question_id: String,
    answer: String,
    response_mode: ResponseMode,
    revision: u32,
}

#[derive(Debug)]
enum ApiError {
    Unauthorized,
    Service(ServiceError),
}

impl From<ServiceError> for ApiError {
    fn from(value: ServiceError) -> Self {
        Self::Service(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Сервисный токен не принят",
            ),
            Self::Service(ServiceError::InvalidInput) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "interview_input_invalid",
                "Параметры интервью некорректны",
            ),
            Self::Service(ServiceError::NotFound) => (
                StatusCode::NOT_FOUND,
                "interview_not_found",
                "Собеседование не найдено",
            ),
            Self::Service(ServiceError::Conflict) => (
                StatusCode::CONFLICT,
                "interview_conflict",
                "Состояние собеседования изменилось; обновите данные",
            ),
            Self::Service(ServiceError::ModelUnavailable) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "interview_model_unavailable",
                "Модель собеседования временно недоступна",
            ),
            Self::Service(ServiceError::Unavailable) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "interview_service_unavailable",
                "Сервис собеседований временно недоступен",
            ),
        };
        (status, Json(ErrorBody { code, message })).into_response()
    }
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::OfflineModel;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_is_public_and_interviews_require_service_token() {
        let app = router(
            Arc::new(InterviewService::new(Arc::new(OfflineModel))),
            "test-token".into(),
        );
        let health = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);
        let unauthorized = app
            .oneshot(
                Request::post("/v1/interviews")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"candidateId":"candidate-1","blueprintId":"backend-middle-v1"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        let body = unauthorized.into_body().collect().await.unwrap().to_bytes();
        assert!(String::from_utf8_lossy(&body).contains("unauthorized"));
    }
}
