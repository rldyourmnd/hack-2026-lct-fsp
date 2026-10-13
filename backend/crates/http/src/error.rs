use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// A contract error. Handlers return it; the transport middleware renders it as
/// `{ code, message, requestId, details }` with the request's ID.
#[derive(Debug, Clone)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    details: Vec<Detail>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Detail {
    pub path: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Envelope<'a> {
    code: &'a str,
    message: &'a str,
    request_id: &'a str,
    details: &'a [Detail],
}

impl ApiError {
    pub(crate) fn code(&self) -> &'static str {
        self.code
    }

    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: Vec::new(),
        }
    }

    pub fn forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, code, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    #[must_use]
    pub fn with_details(mut self, details: Vec<Detail>) -> Self {
        self.details = details;
        self
    }

    pub(crate) fn render(&self, request_id: &str) -> Response {
        let envelope = Envelope {
            code: self.code,
            message: &self.message,
            request_id,
            details: &self.details,
        };
        (self.status, Json(envelope)).into_response()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = self.status.into_response();
        response.extensions_mut().insert(self);
        response
    }
}
