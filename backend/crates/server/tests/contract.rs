//! Contract behavior tests: the real HTTP router over a seeded isolated
//! PostgreSQL database. They mirror the mock's behavior matrix (mock-server's
//! `mock-api.test.ts`) against the Rust implementation.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use fsp_application::{
    InterviewAnswerInput, InterviewGateway, InterviewStartInput, InterviewStatusView,
    InterviewView, InterviewsView, PlatformService, VerificationDelivery,
};
use fsp_http::{HttpConfig, router};
use fsp_integration_fsp_synthetic::HttpFsp;
use fsp_storage_postgres::PostgresRepository;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Admin connection string for the test cluster; `just db-up` provides the
/// matching local PostgreSQL, CI runs the same service container.
fn test_admin_url() -> String {
    std::env::var("FSP_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://fsp:fsp@127.0.0.1:5434/postgres".into())
}

struct FixedDelivery;

struct FixedInterview;

#[async_trait::async_trait]
impl InterviewGateway for FixedInterview {
    async fn list(
        &self,
        _candidate_id: &str,
    ) -> Result<InterviewsView, fsp_application::IntegrationError> {
        Ok(InterviewsView { items: Vec::new() })
    }

    async fn start(
        &self,
        candidate_id: &str,
        input: &InterviewStartInput,
    ) -> Result<InterviewView, fsp_application::IntegrationError> {
        Ok(interview_fixture(candidate_id, &input.blueprint_id, 0))
    }

    async fn get(
        &self,
        _interview_id: &str,
    ) -> Result<InterviewView, fsp_application::IntegrationError> {
        Ok(interview_fixture("candidate024", "backend-middle-v1", 0))
    }

    async fn answer(
        &self,
        _interview_id: &str,
        input: &InterviewAnswerInput,
    ) -> Result<InterviewView, fsp_application::IntegrationError> {
        Ok(interview_fixture(
            "candidate024",
            "backend-middle-v1",
            input.revision + 1,
        ))
    }
}

fn interview_fixture(candidate_id: &str, blueprint_id: &str, revision: u32) -> InterviewView {
    InterviewView {
        id: "interview-test".into(),
        candidate_id: candidate_id.into(),
        blueprint_id: blueprint_id.into(),
        status: InterviewStatusView::Active,
        revision,
        current_question: None,
        nodes: Vec::new(),
        evidence: Vec::new(),
        model_mode: "fixture".into(),
    }
}

#[async_trait::async_trait]
impl VerificationDelivery for FixedDelivery {
    fn code_for(&self) -> Result<String, fsp_application::IntegrationError> {
        Ok("123456".into())
    }

    async fn deliver(
        &self,
        _email: &str,
        _code: &str,
        _challenge_id: &str,
    ) -> Result<(), fsp_application::IntegrationError> {
        Ok(())
    }

    fn contract_name(&self) -> &'static str {
        "mock_mailbox"
    }
}

async fn test_app() -> axum::Router {
    let (repository, _database) =
        PostgresRepository::connect_test_database(&test_admin_url()).await;
    app_with_repository(repository).await
}

async fn app_with_repository(repository: PostgresRepository) -> axum::Router {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, fsp_integration_fsp_synthetic::router())
            .await
            .unwrap();
    });
    app_with_service(
        PlatformService::new(Arc::new(repository), Arc::new(FixedDelivery))
            .with_fsp(Arc::new(HttpFsp::new(&url).unwrap())),
    )
    .await
}

async fn app_with_service(platform: PlatformService) -> axum::Router {
    let service = Arc::new(platform);
    service.initialize().await.expect("seeded demo data");
    router(
        HttpConfig {
            allowed_origins: vec!["http://localhost:5173".into()],
            secure_cookie: false,
            public_web_url: "http://localhost:5173".into(),
            github_callback_url: "http://localhost:8080/api/v1/integrations/github/callback".into(),
            static_dir: None,
        },
        service,
    )
}

fn request(method: &str, path: &str, cookie: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    match body {
        Some(value) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&value).unwrap()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn call(app: &axum::Router, req: Request<Body>) -> (StatusCode, HeaderMap, Value) {
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, headers, body)
}

async fn get(app: &axum::Router, path: &str, cookie: Option<&str>) -> (StatusCode, Value) {
    let (status, _, body) = call(app, request("GET", path, cookie, None)).await;
    (status, body)
}

async fn send(
    app: &axum::Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let (status, _, body) = call(app, request(method, path, cookie, Some(body))).await;
    (status, body)
}

/// The five idempotent commands require `Idempotency-Key` (8..=128 chars).
fn keyed_request(
    method: &str,
    path: &str,
    cookie: Option<&str>,
    key: &str,
    body: Option<Value>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("idempotency-key", key);
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    match body {
        Some(value) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&value).unwrap()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

/// POST without a body — the command side of the bodyless router.
async fn post(app: &axum::Router, path: &str, cookie: Option<&str>) -> (StatusCode, Value) {
    let (status, _, body) = call(app, request("POST", path, cookie, None)).await;
    (status, body)
}

async fn send_keyed(
    app: &axum::Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    key: &str,
    body: Value,
) -> (StatusCode, Value) {
    let (status, _, body) = call(app, keyed_request(method, path, cookie, key, Some(body))).await;
    (status, body)
}

/// The demo code completes `createEmailChallenge` + `verifyEmail` and returns
/// the session cookie pair for following calls.
async fn login(app: &axum::Router, email: &str, role: &str) -> String {
    let (status, body) = send(
        app,
        "POST",
        "/api/v1/auth/email/challenges",
        None,
        json!({"email": email, "role": role}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "challenge: {body}");
    let challenge_id = body["challengeId"].as_str().unwrap().to_owned();
    let (status, headers, body) = call(
        app,
        request(
            "POST",
            "/api/v1/auth/email/verify",
            None,
            Some(json!({"challengeId": challenge_id, "code": "123456"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "verify: {body}");
    let cookie = headers
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .unwrap_or_default()
        .to_owned();
    assert!(cookie.starts_with("fsp_session="), "cookie: {cookie}");
    cookie
}

fn code(body: &Value) -> &str {
    body["code"].as_str().unwrap_or_default()
}

const EMPLOYER: &str = "hr@sever-tech.example";
const CANDIDATE: &str = "candidate031@example.com";
const CANDIDATE_JUNIOR: &str = "alexey@example.com";

/// Regression: a lazily-registered candidate completes the assessment flow.
/// Seeded accounts reuse the candidate id as the account id, so job writes
/// keyed by candidate_id only fail for fresh `actor-*`/`candidate-*` pairs —
/// this exercises the path the seeded fixtures cannot.
#[tokio::test]
async fn fresh_candidate_completes_assessment() {
    let app = test_app().await;
    let cookie = login(&app, "newcomer@example.com", "candidate").await;
    let (status, profile) = send(
        &app,
        "PATCH",
        "/api/v1/me/profile",
        Some(&cookie),
        json!({
            "name": "Новый Кандидат", "selectedGrade": "junior",
            "stack": ["rust"], "experience": "Учебные проекты.",
            "teamRole": "Разработчик",
            "consents": {"processing": true, "publication": true}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{profile}");
    assert_eq!(profile["onboarding"]["questionnaireComplete"], true);
    assert_eq!(profile["onboarding"]["blockers"], json!(["category"]));
    assert_eq!(profile["category"]["source"], Value::Null);
    assert_eq!(profile["estimate"], Value::Null);
    assert_eq!(
        profile["assessmentEligibility"]["selectedReason"],
        Value::Null
    );
    // Without a connected source there is nothing to estimate from.
    let (status, refused) = post(&app, "/api/v1/me/profile/estimate", Some(&cookie)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(code(&refused), "source_not_connected");
    let (status, assessment) = send_keyed(
        &app,
        "POST",
        "/api/v1/me/assessments",
        Some(&cookie),
        "fresh-assess-01",
        json!({"specialization": "backend", "grade": "junior"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{assessment}");
    let assessment_id = assessment["id"].as_str().unwrap().to_owned();
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/v1/me/assessments/{assessment_id}/answer"),
        Some(&cookie),
        knowledge_answer(&assessment["task"], true),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, submit) = call(
        &app,
        keyed_request(
            "POST",
            &format!("/api/v1/me/assessments/{assessment_id}/submit"),
            Some(&cookie),
            "fresh-submit-01",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{submit}");
    let (status, job) = get(
        &app,
        &format!("/api/v1/jobs/{}", submit["id"].as_str().unwrap()),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(job["status"], "succeeded");
    let (status, profile) = get(&app, "/api/v1/me/profile", Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(profile["verifiedGrade"], "junior");
    assert_eq!(profile["verifiedSpecialization"], "backend");
    assert_eq!(profile["category"]["source"], "assessment");
    assert_eq!(profile["category"]["grade"], "junior");
}

#[tokio::test]
async fn transport_envelope_and_boundaries() {
    let app = test_app().await;
    // Unknown path returns the contract envelope, not a bare 404.
    let (status, body) = get(&app, "/api/v1/nowhere", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(code(&body), "not_found");
    assert!(body["requestId"].as_str().is_some());
    // Malformed JSON is `invalid_json`.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/email/challenges")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{oops"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(code(&body), "invalid_json");
    // A schema violation reports field details.
    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/email/challenges",
        None,
        json!({"email": "a@b.c"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&body), "validation_error");
    assert!(body["details"].as_array().is_some_and(|d| !d.is_empty()));
    // Repeating a query key is rejected before handlers run.
    let (status, body) = get(&app, "/api/v1/vacancies?skill=rust&skill=docker", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&body), "duplicate_query_parameter");
}

#[tokio::test]
async fn session_and_role_boundaries() {
    let app = test_app().await;
    let (status, body) = get(&app, "/api/v1/me/profile", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(code(&body), "unauthenticated");
    let employer = login(&app, EMPLOYER, "employer").await;
    let (status, _) = get(&app, "/api/v1/me/profile", Some(&employer)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // A second role on the same mailbox is refused.
    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/email/challenges",
        None,
        json!({"email": EMPLOYER, "role": "candidate"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(code(&body), "role_mismatch");
    // Session survives and resolves the actor.
    let (status, body) = get(&app, "/api/v1/session", Some(&employer)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["role"], "employer");
    assert_eq!(body["emailVerified"], true);
}

#[tokio::test]
async fn email_challenge_lifecycle() {
    let app = test_app().await;
    // A wrong code does not consume the challenge and reports invalid_code.
    let (_, body) = send(
        &app,
        "POST",
        "/api/v1/auth/email/challenges",
        None,
        json!({"email": "new-user@example.com", "role": "candidate"}),
    )
    .await;
    let challenge_id = body["challengeId"].as_str().unwrap();
    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/email/verify",
        None,
        json!({"challengeId": challenge_id, "code": "000000"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&body), "invalid_code");
    // The right code still works afterwards.
    let cookie = login(&app, "new-user@example.com", "candidate").await;
    let (status, body) = get(&app, "/api/v1/me/profile", Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], "new-user@example.com");
    // Unknown challenge id is a 404.
    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/email/verify",
        None,
        json!({"challengeId": "challenge-nope", "code": "123456"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = body;
}

#[tokio::test]
async fn email_attempt_budget_survives_errors_and_reissued_codes() {
    let app = test_app().await;
    let email = json!({"email": "locked@example.com", "role": "candidate"});
    let (_, first) = send(
        &app,
        "POST",
        "/api/v1/auth/email/challenges",
        None,
        email.clone(),
    )
    .await;
    for _ in 0..3 {
        let (status, body) = send(
            &app,
            "POST",
            "/api/v1/auth/email/verify",
            None,
            json!({"challengeId": first["challengeId"], "code": "000000"}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(code(&body), "invalid_code");
    }
    let (status, second) = send(
        &app,
        "POST",
        "/api/v1/auth/email/challenges",
        None,
        email.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    for _ in 0..2 {
        let (status, _) = send(
            &app,
            "POST",
            "/api/v1/auth/email/verify",
            None,
            json!({"challengeId": second["challengeId"], "code": "000000"}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    for challenge in [first, second] {
        let (status, body) = send(
            &app,
            "POST",
            "/api/v1/auth/email/verify",
            None,
            json!({"challengeId": challenge["challengeId"], "code": "123456"}),
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
        assert_eq!(code(&body), "challenge_locked");
    }
    let (status, body) = send(&app, "POST", "/api/v1/auth/email/challenges", None, email).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(code(&body), "challenge_locked");
}

#[tokio::test]
async fn simultaneous_email_verification_reuses_one_account() {
    let app = test_app().await;
    let email = json!({"email": "parallel@example.com", "role": "candidate"});
    let (_, first) = send(
        &app,
        "POST",
        "/api/v1/auth/email/challenges",
        None,
        email.clone(),
    )
    .await;
    let (_, second) = send(&app, "POST", "/api/v1/auth/email/challenges", None, email).await;
    let (a, b) = tokio::join!(
        send(
            &app,
            "POST",
            "/api/v1/auth/email/verify",
            None,
            json!({"challengeId": first["challengeId"], "code": "123456"})
        ),
        send(
            &app,
            "POST",
            "/api/v1/auth/email/verify",
            None,
            json!({"challengeId": second["challengeId"], "code": "123456"})
        ),
    );
    assert_eq!(a.0, StatusCode::OK, "{}", a.1);
    assert_eq!(b.0, StatusCode::OK, "{}", b.1);
    assert_eq!(a.1, b.1);
    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/auth/email/verify",
        None,
        json!({"challengeId": first["challengeId"], "code": "123456"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(code(&body), "challenge_used");
}

#[tokio::test]
async fn consent_gates_sensitive_operations() {
    let app = test_app().await;
    // A fresh candidate has no processing consent: profile import refuses.
    let cookie = login(&app, "fresh@example.com", "candidate").await;
    let boundary = "----fsp-test";
    let payload = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"x.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-1.4 body %%EOF\r\n--{boundary}--\r\n"
    )
    .into_bytes();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/me/profile/imports")
                .header(header::COOKIE, &cookie)
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(code(&body), "processing_consent_required");
}

#[tokio::test]
async fn needs_write_side_and_vacancy_projection() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    // Invalid salary range is a validation error.
    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/needs",
        Some(&employer),
        json!({
            "title": "Backend engineer",
            "specialization": "backend",
            "grade": "middle",
            "workMode": "remote",
            "salary": {"from": 300000, "to": 200000, "currency": "RUB", "period": "month", "taxBasis": "gross"},
            "description": "Описание потребности для проверки валидации.",
            "requirements": [
                {"skillId": "rust", "criticality": "required", "minimumGrade": "middle",
                 "evidencePolicy": {"acceptAssessments": true, "acceptGithub": true, "acceptFsp": true,
                                    "competitionSelection": "system", "competitionIds": []}}
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(code(&body), "salary_range_invalid");
    // A valid draft is created and hidden from the vacancy list until published.
    let (status, draft) = send(
        &app,
        "POST",
        "/api/v1/needs",
        Some(&employer),
        json!({
            "title": "Инженер данных",
            "specialization": "backend",
            "grade": "junior",
            "workMode": "hybrid",
            "salary": {"from": 120000, "to": 180000, "currency": "RUB", "period": "month", "taxBasis": "gross"},
            "description": "Сбор и обработка данных для продуктовой аналитики.",
            "requirements": [
                {"skillId": "postgresql", "criticality": "required", "minimumGrade": "junior",
                 "evidencePolicy": {"acceptAssessments": true, "acceptGithub": false, "acceptFsp": true,
                                    "competitionSelection": "system", "competitionIds": []}}
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{draft}");
    let need_id = draft["id"].as_str().unwrap().to_owned();
    assert_eq!(draft["published"], false);
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let (status, _) = get(
        &app,
        &format!("/api/v1/vacancies/{need_id}"),
        Some(&candidate),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Publishing projects the need into vacancies.
    let (status, body) = post(
        &app,
        &format!("/api/v1/needs/{need_id}/publish"),
        Some(&employer),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, vacancy) = get(
        &app,
        &format!("/api/v1/vacancies/{need_id}"),
        Some(&candidate),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(vacancy["id"], need_id);
    assert!(vacancy["match"].is_object() || vacancy["match"].is_null());
    assert!(vacancy["eligibility"].is_object());
    // Tenant isolation: a candidate cannot read the employer's need draft list.
    let (status, body) = get(&app, "/api/v1/needs", Some(&candidate)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}

#[tokio::test]
async fn source_connection_sync_and_jobs_remain_private() {
    let app = test_app().await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let (status, source) = send(
        &app,
        "POST",
        "/api/v1/me/sources/fsp/connect",
        Some(&candidate),
        json!({"account":"FSP-DEMO-024"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{source}");
    let (status, job) = post(
        &app,
        &format!("/api/v1/me/sources/{}/sync", source["id"].as_str().unwrap()),
        Some(&candidate),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    assert_eq!(job["status"], "succeeded");
    assert_eq!(job["result"]["kind"], "source_sync");
    let job_path = format!("/api/v1/jobs/{}", job["id"].as_str().unwrap());
    assert_eq!(
        get(&app, &job_path, Some(&candidate)).await.0,
        StatusCode::OK
    );
    let outsider = login(&app, "outsider@example.com", "candidate").await;
    assert_eq!(
        get(&app, &job_path, Some(&outsider)).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn concurrent_shortlist_edits_preserve_members_and_company_scope() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    let (status, shortlist) = send(
        &app,
        "POST",
        "/api/v1/shortlists",
        Some(&employer),
        json!({"title":"Кандидаты"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let path = format!("/api/v1/shortlists/{}", shortlist["id"].as_str().unwrap());
    let first = format!("{path}/members/candidate018");
    let second = format!("{path}/members/candidate031");
    let (a, b, c) = tokio::join!(
        call(&app, request("PUT", &first, Some(&employer), None)),
        call(&app, request("PUT", &second, Some(&employer), None)),
        send(
            &app,
            "PATCH",
            &path,
            Some(&employer),
            json!({"title":"Основной список"})
        ),
    );
    assert_eq!(a.0, StatusCode::OK, "{}", a.2);
    assert_eq!(b.0, StatusCode::OK, "{}", b.2);
    assert_eq!(c.0, StatusCode::OK, "{}", c.1);
    let (_, current) = get(&app, &path, Some(&employer)).await;
    assert_eq!(current["title"], "Основной список");
    assert_eq!(current["members"].as_array().unwrap().len(), 2);
    let outsider = login(&app, "other-company@example.com", "employer").await;
    assert_eq!(
        get(&app, &path, Some(&outsider)).await.0,
        StatusCode::NOT_FOUND
    );
    let (deleted, added) = tokio::join!(
        call(&app, request("DELETE", &path, Some(&employer), None)),
        call(&app, request("PUT", &first, Some(&employer), None)),
    );
    assert_eq!(deleted.0, StatusCode::NO_CONTENT);
    assert!([StatusCode::OK, StatusCode::NOT_FOUND].contains(&added.0));
    assert_eq!(
        get(&app, &path, Some(&employer)).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn parallel_withdrawals_revoke_the_last_disclosure_grants() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let (status, published) = post(&app, "/api/v1/needs/need-001/publish", Some(&employer)).await;
    assert_eq!(status, StatusCode::OK, "{published}");
    let (_, session) = get(&app, "/api/v1/session", Some(&candidate)).await;
    let disclosure_path = format!(
        "/api/v1/candidates/{}/disclosure",
        session["candidateId"].as_str().unwrap()
    );
    let mut paths = Vec::new();
    for (need, field) in [("need-001", "name"), ("need-002", "email")] {
        let (status, body) = send_keyed(
            &app,
            "POST",
            "/api/v1/me/applications",
            Some(&candidate),
            &format!("withdraw-{need}"),
            json!({"vacancyId":need,"message":"Отклик","disclose":[field]}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        paths.push(format!(
            "/api/v1/me/applications/{}/withdraw",
            body["id"].as_str().unwrap()
        ));
    }
    let (_, disclosure) = get(&app, &disclosure_path, Some(&employer)).await;
    assert_eq!(disclosure["granted"], true);
    let (a, b) = tokio::join!(
        post(&app, &paths[0], Some(&candidate)),
        post(&app, &paths[1], Some(&candidate))
    );
    assert_eq!(a.0, StatusCode::OK, "{}", a.1);
    assert_eq!(b.0, StatusCode::OK, "{}", b.1);
    let (_, disclosure) = get(&app, &disclosure_path, Some(&employer)).await;
    assert_eq!(disclosure["granted"], false, "{disclosure}");
}

#[tokio::test]
async fn invitations_freeze_terms_and_disclose_on_accept() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    let candidate = login(&app, CANDIDATE, "candidate").await;
    // Idempotent create: the same key replays the stored response.
    let invite = |key: &str| {
        Request::builder()
            .method("POST")
            .uri("/api/v1/invitations")
            .header(header::COOKIE, &employer)
            .header(header::CONTENT_TYPE, "application/json")
            .header("idempotency-key", key)
            .body(Body::from(
                serde_json::to_vec(&json!({
                    "candidateId": "candidate031",
                    "needId": "need-001",
                    "message": "Позиция в основной команде"
                }))
                .unwrap(),
            ))
            .unwrap()
    };
    let (status, _, first) = call(&app, invite("invite-key-0001")).await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    let invitation_id = first["id"].as_str().unwrap().to_owned();
    let (status, headers, replay) = call(&app, invite("invite-key-0001")).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(replay["id"], invitation_id);
    assert_eq!(headers.get("x-idempotent-replay").unwrap(), "true");
    // The candidate lists and accepts the invitation.
    let (status, body) = get(&app, "/api/v1/me/invitations", Some(&candidate)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == invitation_id || item["need"]["id"] == "need-001")
    );
    let view_path = format!("/api/v1/me/invitations/{invitation_id}/view");
    let accept_path = format!("/api/v1/me/invitations/{invitation_id}/accept");
    let ((view_status, _), (status, body)) = tokio::join!(
        post(&app, &view_path, Some(&candidate)),
        send(
            &app,
            "POST",
            &accept_path,
            Some(&candidate),
            json!({"disclose": ["name", "email", "sourceLinks"]}),
        )
    );
    assert_eq!(view_status, StatusCode::OK);
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, current) = get(
        &app,
        &format!("/api/v1/me/invitations/{invitation_id}"),
        Some(&candidate),
    )
    .await;
    assert_eq!(current["status"], "accepted");
    // The company now sees the candidate's disclosure.
    let (status, disclosure) = get(
        &app,
        "/api/v1/candidates/candidate031/disclosure",
        Some(&employer),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(disclosure["granted"], true);
}

#[tokio::test]
async fn applications_flow_both_sides() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    // The published vacancy is reachable and reports eligibility.
    let (status, vacancy) = get(&app, "/api/v1/vacancies/need-002", Some(&candidate)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(vacancy["eligibility"].is_object());
    // Apply with disclosure choices; createApplication is idempotent.
    let (status, application) = send_keyed(
        &app,
        "POST",
        "/api/v1/me/applications",
        Some(&candidate),
        "apply-key-0000001",
        json!({
            "vacancyId": "need-002",
            "message": "Хочу работать над вашими сервисами",
            "disclose": ["name", "email"]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{application}");
    let application_id = application["id"].as_str().unwrap().to_owned();
    // The employer lists the application and accepts it.
    let (status, listing) = get(&app, "/api/v1/applications", Some(&employer)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        listing["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == application_id)
    );
    let (status, decided) = post(
        &app,
        &format!("/api/v1/applications/{application_id}/accept"),
        Some(&employer),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{decided}");
    assert_eq!(decided["status"], "accepted");
    // Withdrawal keeps the terminal decision stable.
    let (status, body) = post(
        &app,
        &format!("/api/v1/me/applications/{application_id}/withdraw"),
        Some(&candidate),
    )
    .await;
    assert!(
        status == StatusCode::OK || status == StatusCode::CONFLICT,
        "{body}"
    );
}

#[tokio::test]
async fn a_private_applicant_is_visible_only_to_the_recipient_until_withdrawal() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let (status, _) = send(
        &app,
        "PATCH",
        "/api/v1/me/profile",
        Some(&candidate),
        json!({"consents":{"processing":true,"publication":false}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let path = "/api/v1/candidates/candidate024?needId=need-002";
    assert_eq!(
        get(&app, path, Some(&employer)).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, application) = send_keyed(
        &app,
        "POST",
        "/api/v1/me/applications",
        Some(&candidate),
        "private-apply-key",
        json!({"vacancyId":"need-002","message":"Interested","disclose":["email"]}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{application}");
    let (status, view) = get(&app, path, Some(&employer)).await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert!(view["match"].is_object());
    assert!(!view.to_string().contains(CANDIDATE_JUNIOR));
    let outsider = login(&app, "other-hr@example.com", "employer").await;
    assert_eq!(
        get(&app, "/api/v1/candidates/candidate024", Some(&outsider))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, _) = post(
        &app,
        &format!(
            "/api/v1/me/applications/{}/withdraw",
            application["id"].as_str().unwrap()
        ),
        Some(&candidate),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        get(&app, path, Some(&employer)).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn assessments_full_flow_and_grade_change() {
    let app = test_app().await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    // candidate-024 is junior-verified since June — a middle attempt is legal.
    let (status, assessment) = send_keyed(
        &app,
        "POST",
        "/api/v1/me/assessments",
        Some(&candidate),
        "assess-key-00001",
        json!({"specialization": "backend", "grade": "middle"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{assessment}");
    let assessment_id = assessment["id"].as_str().unwrap().to_owned();
    assert_eq!(assessment["status"], "draft");
    assert_eq!(assessment["task"]["family"], "backend-knowledge");
    // A second active attempt conflicts.
    let (status, body) = send_keyed(
        &app,
        "POST",
        "/api/v1/me/assessments",
        Some(&candidate),
        "assess-key-00002",
        json!({"specialization": "backend", "grade": "senior"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(code(&body), "assessment_active");
    assert_eq!(assessment["simulation"], false);
    // Answers, checks and assistant.
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/v1/me/assessments/{assessment_id}/answer"),
        Some(&candidate),
        knowledge_answer(&assessment["task"], true),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, checks) = post(
        &app,
        &format!("/api/v1/me/assessments/{assessment_id}/checks"),
        Some(&candidate),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{checks}");
    assert_eq!(checks["kind"], "checks");
    let (status, job) = get(
        &app,
        &format!("/api/v1/jobs/{}", checks["id"].as_str().unwrap()),
        Some(&candidate),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(job["status"], "succeeded");
    assert_eq!(job["result"]["kind"], "checks");
    let (status, answer) = send(
        &app,
        "POST",
        &format!("/api/v1/me/assessments/{assessment_id}/assistant"),
        Some(&candidate),
        json!({"message": "как проверить повтор?"}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{answer}");
    assert_eq!(answer["code"], "assistant_unavailable");
    // Submit is bodyless but keyed; it resolves synchronously in the demo.
    let submit_path = format!("/api/v1/me/assessments/{assessment_id}/submit");
    let ((status, _, submit), (patch_status, patch_result)) = tokio::join!(
        call(
            &app,
            keyed_request(
                "POST",
                &submit_path,
                Some(&candidate),
                "submit-key-00001",
                None,
            ),
        ),
        send(
            &app,
            "PATCH",
            "/api/v1/me/profile",
            Some(&candidate),
            json!({"name":"Обновлённое имя"})
        )
    );
    assert_eq!(patch_status, StatusCode::OK, "{patch_result}");
    assert_eq!(status, StatusCode::ACCEPTED, "{submit}");
    let (status, job) = get(
        &app,
        &format!("/api/v1/jobs/{}", submit["id"].as_str().unwrap()),
        Some(&candidate),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(job["status"], "succeeded");
    assert_eq!(job["result"]["kind"], "assessment");
    assert_eq!(job["result"]["assessment"]["status"], "passed");
    let (_, profile) = get(&app, "/api/v1/me/profile", Some(&candidate)).await;
    assert_eq!(profile["verifiedGrade"], "middle");
    assert_eq!(profile["name"], "Обновлённое имя");
    let (_, skills) = get(&app, "/api/v1/me/skills", Some(&candidate)).await;
    assert!(
        skills["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["skillId"] == "rust" || s["skillId"] == "postgresql")
            .all(|s| s["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["reference"] == assessment_id && e["synthetic"] == false))
    );
}

#[tokio::test]
async fn graph_interview_is_scoped_to_the_authenticated_candidate() {
    let (repository, _database) =
        PostgresRepository::connect_test_database(&test_admin_url()).await;
    let app = app_with_service(
        PlatformService::new(Arc::new(repository), Arc::new(FixedDelivery))
            .with_interviews(Arc::new(FixedInterview)),
    )
    .await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let (status, interview) = send(
        &app,
        "POST",
        "/api/v1/me/interviews",
        Some(&candidate),
        json!({"blueprintId":"backend-middle-v1","maxTurns":10}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{interview}");
    assert_eq!(interview["candidateId"], "candidate024");
    let (status, _) = send(
        &app,
        "POST",
        "/api/v1/me/interviews/interview-test/answers",
        Some(&candidate),
        json!({"questionId":"question-1","answer":"Ответ","responseMode":"text","revision":0}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        get(&app, "/api/v1/me/interviews", Some(&candidate)).await.0,
        StatusCode::OK
    );
    let other_candidate = login(&app, CANDIDATE, "candidate").await;
    let (status, body) = get(
        &app,
        "/api/v1/me/interviews/interview-test",
        Some(&other_candidate),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let employer = login(&app, EMPLOYER, "employer").await;
    assert_eq!(
        get(&app, "/api/v1/me/interviews", Some(&employer)).await.0,
        StatusCode::FORBIDDEN
    );

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/v1/me/profile",
        Some(&candidate),
        json!({"consents":{"processing":false,"publication":false}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for (path, body) in [
        (
            "/api/v1/me/interviews",
            json!({"blueprintId":"backend-middle-v1","maxTurns":10}),
        ),
        (
            "/api/v1/me/interviews/interview-test/answers",
            json!({"questionId":"question-1","answer":"Ответ","responseMode":"text","revision":0}),
        ),
    ] {
        let (status, body) = send(&app, "POST", path, Some(&candidate), body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(code(&body), "processing_consent_required");
    }
    // Revocation stops new processing, not access to the candidate's own history.
    assert_eq!(
        get(
            &app,
            "/api/v1/me/interviews/interview-test",
            Some(&candidate)
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, _) = send(
        &app,
        "POST",
        "/api/v1/me/interviews/interview-test/answers",
        Some(&other_candidate),
        json!({"questionId":"question-1","answer":"Ответ","responseMode":"text","revision":0}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn assessment_guards_cooldown_and_downgrade() {
    let app = test_app().await;
    let senior = login(&app, CANDIDATE, "candidate").await;
    // candidate031 is senior-verified: a lower-grade attempt is a downgrade.
    let (status, body) = send_keyed(
        &app,
        "POST",
        "/api/v1/me/assessments",
        Some(&senior),
        "assess-key-00003",
        json!({"specialization": "backend", "grade": "junior"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(code(&body), "grade_downgrade_forbidden");
    // Every catalog specialization and grade has a test now; other guards may still apply.
    let (_, body) = send_keyed(
        &app,
        "POST",
        "/api/v1/me/assessments",
        Some(&senior),
        "assess-key-00004",
        json!({"specialization": "frontend", "grade": "senior"}),
    )
    .await;
    assert_ne!(code(&body), "assessment_scope_unavailable");
}

#[tokio::test]
async fn idempotency_replays_and_conflicts() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    // createInvitation is one of the five idempotent commands.
    let body = json!({
        "candidateId": "candidate018",
        "needId": "need-001",
        "message": "Предложение по вакансии"
    });
    let invite = |key: &str, body: Value| {
        keyed_request(
            "POST",
            "/api/v1/invitations",
            Some(&employer),
            key,
            Some(body),
        )
    };
    let (first_call, second_call) = tokio::join!(
        call(&app, invite("idem-key-00001", body.clone())),
        call(&app, invite("idem-key-00001", body.clone())),
    );
    let ((status, _, first), (second_status, headers, second)) =
        if first_call.1.contains_key("x-idempotent-replay") {
            (second_call, first_call)
        } else {
            (first_call, second_call)
        };
    assert_eq!(status, StatusCode::CREATED, "{first}");
    assert_eq!(second_status, StatusCode::CREATED, "{second}");
    assert_eq!(first, second);
    assert_eq!(headers.get("x-idempotent-replay").unwrap(), "true");
    let (status, headers, replay) = call(&app, invite("idem-key-00001", body.clone())).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(replay["id"], first["id"]);
    assert_eq!(headers.get("x-idempotent-replay").unwrap(), "true");
    // Same key, different payload → conflict before any work.
    let (status, _, body) = call(
        &app,
        invite(
            "idem-key-00001",
            json!({
                "candidateId": "candidate041",
                "needId": "need-001",
                "message": "Другое сообщение"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(code(&body), "idempotency_conflict");
    // Missing key on an idempotent command is a schema violation.
    let (status, _, body) = call(
        &app,
        request(
            "POST",
            "/api/v1/invitations",
            Some(&employer),
            Some(json!({
                "candidateId": "candidate018",
                "needId": "need-001",
                "message": "Без ключа"
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&body), "validation_error");
}

#[tokio::test]
async fn gap_plans_and_reachability() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    // Reachable public candidate against the company's need.
    let (status, plan) = get(
        &app,
        "/api/v1/candidates/candidate018/gap-plan?needId=need-001",
        Some(&employer),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{plan}");
    assert_eq!(plan["simulation"], true);
    let item = plan["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["skillId"] == "postgresql")
        .expect("postgresql gap item");
    assert_eq!(item["status"], "not_evidenced");
    assert_eq!(item["assessmentAvailable"], true);
    assert!(
        item["acceptedCompetitionIds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "competition-demo-api")
    );
    // Another company's need id is not found.
    let (status, body) = get(
        &app,
        "/api/v1/candidates/candidate018/gap-plan?needId=need-stranger",
        Some(&employer),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = body;
}

#[tokio::test]
async fn vacancy_search_pagination() {
    let app = test_app().await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let (status, page) = get(
        &app,
        "/api/v1/vacancies?page=1&pageSize=1",
        Some(&candidate),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["page"], 1);
    assert_eq!(page["pageSize"], 1);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert!(page["total"].as_u64().unwrap() >= 1);
    assert!(page["hasNextPage"].is_boolean());
    // Out-of-range size is a contract violation.
    let (status, body) = get(&app, "/api/v1/vacancies?pageSize=0", Some(&candidate)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&body), "validation_error");
}

#[tokio::test]
async fn tasks_offer_and_response() {
    let app = test_app().await;
    let candidate = login(&app, CANDIDATE, "candidate").await;
    let employer = login(&app, EMPLOYER, "employer").await;
    // MyTasks is the weekly-offer envelope: period, current, history, freshness.
    let (status, tasks) = get(&app, "/api/v1/me/tasks", Some(&candidate)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(tasks["period"].is_object());
    assert!(tasks["history"].is_array());
    assert!(tasks["freshness"].is_object());
    // Company tasks are listed with response counts.
    let (status, list) = get(&app, "/api/v1/tasks", Some(&employer)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == "task-001")
    );
    // Archiving removes it from offers.
    let (status, archived) = post(&app, "/api/v1/tasks/task-001/archive", Some(&employer)).await;
    assert_eq!(status, StatusCode::OK, "{archived}");
}

#[tokio::test]
async fn profile_import_confirm_and_pdf_export() {
    let (repository, _database) =
        PostgresRepository::connect_test_database(&test_admin_url()).await;
    let app = app_with_service(
        PlatformService::new(Arc::new(repository), Arc::new(FixedDelivery))
            .with_pdf(Arc::new(fsp_integration_pdf::SandboxedPdf::default()))
            .with_llm(Arc::new(TestAnalysis)),
    )
    .await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    // Real PDF bytes traverse the sandboxed parser; the analysis double only selects
    // a verbatim source line so this test has no model/network dependency.
    let (_, original) = get(&app, "/api/v1/me/profile", Some(&candidate)).await;
    let exported = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/me/profile/export",
            Some(&candidate),
            None,
        ))
        .await
        .unwrap();
    let pdf = exported.into_body().collect().await.unwrap().to_bytes();
    let boundary = "----fsp-test";
    let mut payload = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"cv.pdf\"\r\nContent-Type: application/pdf\r\n\r\n").into_bytes();
    payload.extend_from_slice(&pdf);
    payload.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/me/profile/imports")
                .header(header::COOKIE, &candidate)
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(payload.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let job: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(job["kind"], "profile_import");
    assert_eq!(job["status"], "succeeded");
    assert_eq!(job["result"]["parser"], "poppler_bifrost");
    assert_eq!(job["simulation"], false);
    assert_eq!(job["result"]["simulation"], false);
    assert_eq!(job["result"]["evidence"][0]["page"], 1);
    let job_id = job["id"].as_str().unwrap().to_owned();
    // The job is replayable through the polling endpoint.
    let (status, polled) = get(&app, &format!("/api/v1/jobs/{job_id}"), Some(&candidate)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(polled["result"]["preview"]["name"], original["name"]);
    assert_eq!(polled["result"]["preview"]["stack"], original["stack"]);
    // Confirm applies the patch once.
    let (status, profile) = send(
        &app,
        "POST",
        &format!("/api/v1/me/profile/imports/{job_id}/confirm"),
        Some(&candidate),
        json!({"profile": {"name": "Импортированный Профиль"}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{profile}");
    assert_eq!(profile["name"], "Импортированный Профиль");
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/v1/me/profile/imports/{job_id}/confirm"),
        Some(&candidate),
        json!({"profile": {"name": "Повтор"}}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(code(&body), "import_already_confirmed");
    // A preview is bound to its profile revision: do not overwrite a newer edit.
    let (status, _, fresh_job) = call(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/me/profile/imports")
            .header(header::COOKIE, &candidate)
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(payload))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{fresh_job}");
    let (status, changed) = send(
        &app,
        "PATCH",
        "/api/v1/me/profile",
        Some(&candidate),
        json!({"name": "Новое имя"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    let (status, conflict) = send(
        &app,
        "POST",
        &format!(
            "/api/v1/me/profile/imports/{}/confirm",
            fresh_job["id"].as_str().unwrap()
        ),
        Some(&candidate),
        json!({"profile": {"name": "Устаревшее имя"}}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
    assert_eq!(conflict["code"], "import_profile_changed");
    let (_, unchanged) = get(&app, "/api/v1/me/profile", Some(&candidate)).await;
    assert_eq!(unchanged["name"], "Новое имя");
    // Export returns a real PDF.
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/me/profile/export",
            Some(&candidate),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/pdf"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 1000);
}

#[tokio::test]
async fn candidate_bank_anonymous_and_sorted() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    let (status, page) = get(
        &app,
        "/api/v1/candidates?page=1&pageSize=10",
        Some(&employer),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = page["items"].as_array().unwrap();
    assert!(!items.is_empty());
    // Anonymous projection: aliases, never e-mail addresses.
    for item in items {
        assert!(item["alias"].as_str().is_some());
        assert!(item.get("email").is_none());
    }
    // The fsp=verified filter narrows to verified candidates.
    let (status, verified) = get(
        &app,
        "/api/v1/candidates?fsp=verified&page=1&pageSize=10",
        Some(&employer),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let verified_items = verified["items"].as_array().unwrap();
    assert!(
        verified_items
            .iter()
            .all(|item| item["fspVerified"] == true)
    );
    // Seeded categories are test-confirmed; the confirmation filter separates them from estimates.
    let (status, confirmed) = get(
        &app,
        "/api/v1/candidates?categorySource=assessment&page=1&pageSize=10",
        Some(&employer),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(confirmed["total"], page["total"]);
    for item in confirmed["items"].as_array().unwrap() {
        assert_eq!(item["categorySource"], "assessment");
        assert_eq!(item["grade"], item["verifiedGrade"]);
        assert_eq!(item["estimate"], Value::Null);
    }
    let (status, estimated) = get(
        &app,
        "/api/v1/candidates?categorySource=estimate",
        Some(&employer),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(estimated["total"], 0);
    let (status, invalid) = get(
        &app,
        "/api/v1/candidates?categorySource=guess",
        Some(&employer),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}");
}

#[tokio::test]
async fn company_patch_rejects_invalid_fields_without_changing_stored_data() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    let (_, before) = get(&app, "/api/v1/company", Some(&employer)).await;
    for email in [
        "x@",
        "a..b@example.com",
        "a@-example.com",
        "name with space@example.com",
    ] {
        let (status, _) = send(
            &app,
            "POST",
            "/api/v1/auth/email/challenges",
            None,
            json!({"email": email, "role": "candidate"}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    for patch in [
        json!({"displayName": "x".repeat(121)}),
        json!({"businessArea": "x".repeat(201)}),
        json!({"description": "x".repeat(4001)}),
        json!({"contactEmail": "not-an-address"}),
        json!({"contactEmail": "x@"}),
        json!({"website": "javascript:alert(1)"}),
        json!({"website": "https://"}),
        json!({"website": "https://example.com/".to_owned() + &"x".repeat(2048)}),
        json!({"displayName": null, "description": "must not be stored"}),
    ] {
        let (status, body) = send(&app, "PATCH", "/api/v1/company", Some(&employer), patch).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(code(&body), "validation_error");
    }
    let (_, after) = get(&app, "/api/v1/company", Some(&employer)).await;
    assert_eq!(after, before);
    let (status, body) = send(&app, "PATCH", "/api/v1/company", Some(&employer), json!({
        "displayName": "я".repeat(120), "businessArea": "x".repeat(200),
        "description": "x".repeat(4000), "contactEmail": "hr+jobs@example.com", "website": "https://example.com/jobs"
    })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn nullable_patch_fields_clear_values_while_omission_preserves_them() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    let (status, _) = send(
        &app,
        "PATCH",
        "/api/v1/company",
        Some(&employer),
        json!({"website": "https://example.com"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, company) = send(
        &app,
        "PATCH",
        "/api/v1/company",
        Some(&employer),
        json!({"description": "Updated"}),
    )
    .await;
    assert_eq!(company["website"], "https://example.com");
    let (status, body) = send(
        &app,
        "PATCH",
        "/api/v1/company",
        Some(&employer),
        json!({"website": null}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, company) = get(&app, "/api/v1/company", Some(&employer)).await;
    assert!(company["website"].is_null());

    let candidate = login(&app, CANDIDATE, "candidate").await;
    let (status, _) = send(
        &app,
        "PATCH",
        "/api/v1/me/profile",
        Some(&candidate),
        json!({"experienceMonths": 24}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, profile) = send(
        &app,
        "PATCH",
        "/api/v1/me/profile",
        Some(&candidate),
        json!({"teamRole": "Developer"}),
    )
    .await;
    assert_eq!(profile["experienceMonths"], 24);
    let (status, body) = send(
        &app,
        "PATCH",
        "/api/v1/me/profile",
        Some(&candidate),
        json!({"experienceMonths": null}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, profile) = get(&app, "/api/v1/me/profile", Some(&candidate)).await;
    assert!(profile["experienceMonths"].is_null());
    let (status, _) = send(
        &app,
        "PATCH",
        "/api/v1/me/profile",
        Some(&candidate),
        json!({"name": null, "teamRole": "Ignored"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = send(
        &app,
        "PATCH",
        "/api/v1/needs/need-001",
        Some(&employer),
        json!({"title": "Ignored", "salary": null}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn concurrent_assessment_starts_with_distinct_keys_return_domain_conflicts() {
    let (repository, _database) =
        PostgresRepository::connect_test_database(&test_admin_url()).await;
    let pool = repository.pool().clone();
    let app = app_with_repository(repository).await;
    // Force overlapping inserts in this isolated database, rather than relying
    // on the local machine to happen to schedule two fast requests together.
    sqlx::raw_sql("CREATE FUNCTION delay_assessment_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(0.2); RETURN NEW; END $$; CREATE TRIGGER delay_assessment_insert BEFORE INSERT ON assessments FOR EACH ROW EXECUTE FUNCTION delay_assessment_insert();")
        .execute(&pool).await.unwrap();
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let mut requests = tokio::task::JoinSet::new();
    for index in 0..8 {
        let app = app.clone();
        let cookie = candidate.clone();
        requests.spawn(async move {
            send_keyed(
                &app,
                "POST",
                "/api/v1/me/assessments",
                Some(&cookie),
                &format!("start-race-key-{index:04}"),
                json!({"specialization": "backend", "grade": "junior"}),
            )
            .await
        });
    }
    let mut created = 0;
    while let Some(result) = requests.join_next().await {
        let (status, body) = result.unwrap();
        if status == StatusCode::CREATED {
            created += 1;
        } else {
            assert_eq!(status, StatusCode::CONFLICT, "{body}");
            assert_eq!(code(&body), "assessment_active");
        }
    }
    assert_eq!(created, 1);
    let (_, listing) = get(&app, "/api/v1/me/assessments", Some(&candidate)).await;
    assert_eq!(
        listing["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["status"] == "draft")
            .count(),
        1
    );
}

struct TestAnalysis;
#[async_trait::async_trait]
impl fsp_application::LlmGateway for TestAnalysis {
    async fn extract_resume(
        &self,
        pages: &[fsp_domain::PdfPage],
    ) -> Result<fsp_domain::ResumeExtraction, fsp_application::IntegrationError> {
        let first = pages[0].text.lines().next().unwrap().trim().to_string();
        Ok(fsp_domain::ResumeExtraction {
            fields: vec![fsp_domain::ResumeField {
                field: "name".into(),
                value: first.clone(),
                page: 1,
                quote: first,
            }],
        })
    }
    async fn assessment_help(
        &self,
        request: &fsp_domain::AssessmentHelpRequest,
    ) -> Result<fsp_domain::AssessmentHelpReply, fsp_application::IntegrationError> {
        Ok(fsp_domain::AssessmentHelpReply {
            message: format!(
                "{}: {}",
                request.question,
                serde_json::to_string(&request.answer).unwrap()
            ),
        })
    }
    async fn match_repository(
        &self,
        _: &fsp_domain::RepositoryMatchRequest,
    ) -> Result<fsp_domain::RepositoryCompetitionMatch, fsp_application::IntegrationError> {
        unreachable!()
    }
}

#[tokio::test]
async fn assistant_uses_saved_answer_and_never_issues_a_grade() {
    let (repository, _database) =
        PostgresRepository::connect_test_database(&test_admin_url()).await;
    let app = app_with_service(
        PlatformService::new(Arc::new(repository), Arc::new(FixedDelivery))
            .with_llm(Arc::new(TestAnalysis)),
    )
    .await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let (status, _, assessment) = call(
        &app,
        keyed_request(
            "POST",
            "/api/v1/me/assessments",
            Some(&candidate),
            "assistant-test-start",
            Some(json!({"specialization":"backend","grade":"middle"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{assessment}");
    let id = assessment["id"].as_str().unwrap();
    let (status, saved) = send(
        &app,
        "PUT",
        &format!("/api/v1/me/assessments/{id}/answer"),
        Some(&candidate),
        knowledge_answer(&assessment["task"], true),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, reply) = send(
        &app,
        "POST",
        &format!("/api/v1/me/assessments/{id}/assistant"),
        Some(&candidate),
        json!({"message":"Как проверить откат?"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    assert_eq!(reply["simulation"], false);
    // The variant (and so the questions) depends on the assessment id; the saved answer names them.
    let question = assessment["task"]["questions"][0]["id"].as_str().unwrap();
    assert!(reply["message"].as_str().unwrap().contains(question));
    assert!(
        reply["message"]
            .as_str()
            .unwrap()
            .contains("Как проверить откат?")
    );
    let (_, unchanged) = get(
        &app,
        &format!("/api/v1/me/assessments/{id}"),
        Some(&candidate),
    )
    .await;
    assert_eq!(unchanged["status"], "in_progress");
    assert_eq!(unchanged["result"], Value::Null);
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/v1/me/assessments/{id}/assistant"),
        Some(&candidate),
        json!({"message":" "}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

fn knowledge_answer(task: &Value, correct: bool) -> Value {
    let legacy: Value =
        serde_json::from_str(include_str!("../../domain/data/knowledge-v1.json")).unwrap();
    let additions: Value = serde_json::from_str(include_str!(
        "../../domain/data/knowledge-v2-additions.json"
    ))
    .unwrap();
    let questions = legacy["questions"]
        .as_array()
        .unwrap()
        .iter()
        .chain(additions["questions"].as_array().unwrap());
    let responses: Vec<Value> = task["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|public| {
            let q = questions
                .clone()
                .find(|candidate| candidate["id"] == public["id"])
                .unwrap();
            let option = if correct {
                &q["correctOptionId"]
            } else {
                &q["options"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|o| o["id"] != q["correctOptionId"])
                    .unwrap()["id"]
            };
            json!({"questionId": q["id"], "optionId": option})
        })
        .collect();
    json!({"code":"", "explanation":"", "externalAiUsed":true, "responses":responses})
}

#[tokio::test]
async fn knowledge_test_rejects_forged_answers_and_scores_failure_without_changing_grade() {
    let app = test_app().await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let (_, assessment) = send_keyed(
        &app,
        "POST",
        "/api/v1/me/assessments",
        Some(&candidate),
        "quiz-negative-start",
        json!({"specialization":"backend", "grade":"middle"}),
    )
    .await;
    let id = assessment["id"].as_str().unwrap();
    let path = format!("/api/v1/me/assessments/{id}");
    assert_eq!(
        assessment["task"]["questions"].as_array().unwrap().len(),
        10
    );
    assert!(!assessment.to_string().contains("correctOptionId"));
    assert!(!assessment.to_string().contains("explanation\":\"Верный"));
    for answer in [
        json!({"code":"arbitrary code", "explanation":"anything", "externalAiUsed":false}),
        json!({"code":"", "explanation":"", "externalAiUsed":false, "responses":[{"questionId":"foreign", "optionId":"x"}]}),
        json!({"code":"", "explanation":"", "externalAiUsed":false, "responses":[{"questionId":"mr-arc", "optionId":"forged"}]}),
    ] {
        let (status, _) = send(
            &app,
            "PUT",
            &format!("{path}/answer"),
            Some(&candidate),
            answer,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let mut duplicate = knowledge_answer(&assessment["task"], false);
    duplicate["responses"][1] = duplicate["responses"][0].clone();
    let (status, _) = send(
        &app,
        "PUT",
        &format!("{path}/answer"),
        Some(&candidate),
        duplicate,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &app,
        keyed_request(
            "POST",
            &format!("{path}/submit"),
            Some(&candidate),
            "incomplete-quiz-submit",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = send(
        &app,
        "PUT",
        &format!("{path}/answer"),
        Some(&candidate),
        knowledge_answer(&assessment["task"], false),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, submitted) = call(
        &app,
        keyed_request(
            "POST",
            &format!("{path}/submit"),
            Some(&candidate),
            "failed-quiz-submit",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{submitted}");
    assert_eq!(submitted["simulation"], false);
    let result = &submitted["result"]["assessment"]["result"];
    assert_eq!(result["outcome"], "failed");
    assert_eq!(result["confirmedGrade"], "junior");
    assert_eq!(result["gradeChanged"], false);
    assert!(
        result["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["status"] == "failed")
    );
    let (_, _, replay) = call(
        &app,
        keyed_request(
            "POST",
            &format!("{path}/submit"),
            Some(&candidate),
            "failed-quiz-submit",
            None,
        ),
    )
    .await;
    assert_eq!(submitted, replay);
    let (status, _) = send_keyed(
        &app,
        "POST",
        "/api/v1/me/assessments",
        Some(&candidate),
        "retry-failed-quiz",
        json!({"specialization":"backend", "grade":"middle"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn new_test_archives_legacy_draft_without_issuing_a_grade() {
    let (repository, _database) =
        PostgresRepository::connect_test_database(&test_admin_url()).await;
    let pool = repository.pool().clone();
    let app = app_with_repository(repository).await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let (_, first) = send_keyed(
        &app,
        "POST",
        "/api/v1/me/assessments",
        Some(&candidate),
        "legacy-start",
        json!({"specialization":"backend","grade":"middle"}),
    )
    .await;
    let id = first["id"].as_str().unwrap();
    sqlx::query("UPDATE assessments SET task_json = jsonb_set(task_json::jsonb, '{execution}', '\"simulated\"')::text WHERE id=$1").bind(id).execute(&pool).await.unwrap();
    let (status, _, result) = call(
        &app,
        keyed_request(
            "POST",
            &format!("/api/v1/me/assessments/{id}/submit"),
            Some(&candidate),
            "legacy-submit",
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result["code"], "assessment_format_retired");
    let (status, second) = send_keyed(
        &app,
        "POST",
        "/api/v1/me/assessments",
        Some(&candidate),
        "new-quiz-start",
        json!({"specialization":"backend","grade":"middle"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{second}");
    let (_, archived) = get(
        &app,
        &format!("/api/v1/me/assessments/{id}"),
        Some(&candidate),
    )
    .await;
    assert_eq!(archived["result"]["outcome"], "service_error");
    assert_eq!(archived["result"]["gradeChanged"], false);
    assert_eq!(archived["result"]["confirmedGrade"], "junior");
}

#[tokio::test]
async fn fsp_http_sync_preserves_snapshot_on_failure_and_never_changes_grade() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let provider = tokio::spawn(async move {
        axum::serve(listener, fsp_integration_fsp_synthetic::router())
            .await
            .unwrap();
    });
    let (repository, _database) =
        PostgresRepository::connect_test_database(&test_admin_url()).await;
    let app = app_with_service(
        PlatformService::new(Arc::new(repository), Arc::new(FixedDelivery))
            .with_fsp(Arc::new(HttpFsp::new(&url).unwrap())),
    )
    .await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let (status, linked) = send(
        &app,
        "POST",
        "/api/v1/me/sources/fsp/connect",
        Some(&candidate),
        json!({"account":"fsp-demo-031"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{linked}");
    // The demo history: four published results (team and individual) and one pending.
    let results = linked["fspResults"].as_array().unwrap();
    assert_eq!(results.len(), 5);
    assert_eq!(results[0]["scope"], "team");
    assert!(
        results
            .iter()
            .any(|result| result["scope"] == "individual" && result["status"] == "published")
    );
    assert_eq!(results[4]["status"], "pending");
    assert_eq!(linked["competitionIds"].as_array().unwrap().len(), 4);
    let (_, before) = get(&app, "/api/v1/me/sources", Some(&candidate)).await;
    let before = before["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["provider"] == "fsp")
        .unwrap()
        .clone();
    provider.abort();
    let _ = provider.await;
    let (status, job) = post(
        &app,
        &format!("/api/v1/me/sources/{}/sync", linked["id"].as_str().unwrap()),
        Some(&candidate),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    assert_eq!(job["status"], "failed");
    assert_eq!(job["error"]["code"], "fsp_unavailable");
    let (_, sources) = get(&app, "/api/v1/me/sources", Some(&candidate)).await;
    let unchanged = sources["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["provider"] == "fsp")
        .unwrap();
    assert_eq!(unchanged, &before);
    let (_, profile) = get(&app, "/api/v1/me/profile", Some(&candidate)).await;
    assert_eq!(profile["verifiedGrade"], "junior");
}

struct MailDelivery {
    sends: std::sync::atomic::AtomicUsize,
    fail: bool,
}
#[async_trait::async_trait]
impl VerificationDelivery for MailDelivery {
    fn code_for(&self) -> Result<String, fsp_application::IntegrationError> {
        Ok("654321".into())
    }
    async fn deliver(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<(), fsp_application::IntegrationError> {
        self.sends.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.fail {
            Err(fsp_application::IntegrationError {
                code: "verification_delivery_failed",
                message: "Не удалось отправить письмо".into(),
            })
        } else {
            Ok(())
        }
    }
    fn contract_name(&self) -> &'static str {
        "email"
    }
}

#[tokio::test]
async fn production_email_throttles_concurrent_delivery_and_keeps_failed_send_budget() {
    for fail in [false, true] {
        let (repo, _db) = PostgresRepository::connect_test_database(&test_admin_url()).await;
        let delivery = Arc::new(MailDelivery {
            sends: 0.into(),
            fail,
        });
        let app = app_with_service(
            PlatformService::new(Arc::new(repo.clone()), delivery.clone())
                .with_auth_key(&"ab".repeat(32))
                .unwrap(),
        )
        .await;
        let email = json!({"email":"rate@example.com","role":"candidate"});
        let (a, b) = tokio::join!(
            send(
                &app,
                "POST",
                "/api/v1/auth/email/challenges",
                None,
                email.clone()
            ),
            send(
                &app,
                "POST",
                "/api/v1/auth/email/challenges",
                None,
                email.clone()
            )
        );
        assert!(a.0 == StatusCode::TOO_MANY_REQUESTS || b.0 == StatusCode::TOO_MANY_REQUESTS);
        assert!(
            a.0 == if fail {
                StatusCode::BAD_GATEWAY
            } else {
                StatusCode::CREATED
            } || b.0
                == if fail {
                    StatusCode::BAD_GATEWAY
                } else {
                    StatusCode::CREATED
                }
        );
        assert_eq!(delivery.sends.load(std::sync::atomic::Ordering::SeqCst), 1);
        for _ in 1..5 {
            sqlx::query("UPDATE challenges SET created_at=created_at-interval '61 seconds' WHERE email='rate@example.com'").execute(repo.pool()).await.unwrap();
            let (status, _) = send(
                &app,
                "POST",
                "/api/v1/auth/email/challenges",
                None,
                email.clone(),
            )
            .await;
            assert_eq!(
                status,
                if fail {
                    StatusCode::BAD_GATEWAY
                } else {
                    StatusCode::CREATED
                }
            );
        }
        sqlx::query("UPDATE challenges SET created_at=created_at-interval '61 seconds' WHERE email='rate@example.com'").execute(repo.pool()).await.unwrap();
        let (status, body) = send(&app, "POST", "/api/v1/auth/email/challenges", None, email).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "rate_limited");
        assert_eq!(delivery.sends.load(std::sync::atomic::Ordering::SeqCst), 5);
    }
}

#[tokio::test]
async fn production_secret_invalidates_demo_credentials_and_rotates_sessions() {
    let (repo, _db) = PostgresRepository::connect_test_database(&test_admin_url()).await;
    let demo = app_with_repository(repo.clone()).await;
    let old_cookie = login(&demo, "existing@example.com", "candidate").await;
    let (_, old_challenge) = send(
        &demo,
        "POST",
        "/api/v1/auth/email/challenges",
        None,
        json!({"email":"pending@example.com","role":"candidate"}),
    )
    .await;
    let make = |key: String| {
        PlatformService::new(
            Arc::new(repo.clone()),
            Arc::new(MailDelivery {
                sends: 0.into(),
                fail: false,
            }),
        )
        .with_auth_key(&key)
        .unwrap()
    };
    let app = app_with_service(make("ab".repeat(32))).await;
    assert_eq!(
        get(&app, "/api/v1/session", Some(&old_cookie)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            &app,
            "POST",
            "/api/v1/auth/email/verify",
            None,
            json!({"challengeId":old_challenge["challengeId"],"code":"123456"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, challenge) = send(
        &app,
        "POST",
        "/api/v1/auth/email/challenges",
        None,
        json!({"email":"secure@example.com","role":"candidate"}),
    )
    .await;
    assert_eq!(challenge["delivery"], "email");
    let payload = json!({"challengeId":challenge["challengeId"],"code":"654321"});
    let (status, headers, _) = call(
        &app,
        request(
            "POST",
            "/api/v1/auth/email/verify",
            None,
            Some(payload.clone()),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let cookie = headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert_eq!(
        get(&app, "/api/v1/session", Some(cookie)).await.0,
        StatusCode::OK
    );
    assert_eq!(
        send(&app, "POST", "/api/v1/auth/email/verify", None, payload)
            .await
            .1["code"],
        "challenge_used"
    );
    let rotated = app_with_service(make("cd".repeat(32))).await;
    assert_eq!(
        get(&rotated, "/api/v1/session", Some(cookie)).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn vacancy_fit_is_identical_across_roles_and_policy_changes() {
    let app = test_app().await;
    let employer = login(&app, EMPLOYER, "employer").await;
    let candidate = login(&app, CANDIDATE_JUNIOR, "candidate").await;
    let (_, profile) = get(&app, "/api/v1/me/profile", Some(&candidate)).await;
    let (_, need) = get(&app, "/api/v1/needs/need-001", Some(&employer)).await;
    let (status, published) = post(&app, "/api/v1/needs/need-001/publish", Some(&employer)).await;
    assert_eq!(status, StatusCode::OK, "{published}");
    for accept_assessments in [true, false] {
        let mut requirements = need["requirements"].clone();
        for requirement in requirements.as_array_mut().unwrap() {
            requirement["evidencePolicy"]["acceptAssessments"] = json!(accept_assessments);
            requirement["evidencePolicy"]["acceptGithub"] = json!(true);
        }
        let (status, updated) = send(
            &app,
            "PATCH",
            "/api/v1/needs/need-001",
            Some(&employer),
            json!({"requirements": requirements}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{updated}");
        let (_, own) = get(&app, "/api/v1/vacancies/need-001", Some(&candidate)).await;
        let path = format!(
            "/api/v1/candidates/{}?needId=need-001",
            profile["id"].as_str().unwrap()
        );
        let (status, seen) = get(&app, &path, Some(&employer)).await;
        assert_eq!(status, StatusCode::OK, "{seen}");
        assert_eq!(seen["match"], own["match"]);
        let requirements = seen["match"]["requirements"].as_array().unwrap();
        let confirmed = requirements
            .iter()
            .filter(|item| item["criticality"] == "required" && item["status"] == "meets")
            .count();
        assert_eq!(seen["match"]["requiredMet"], confirmed);
        if !accept_assessments {
            assert_eq!(confirmed, 0);
        }
        for field in [
            "email",
            "name",
            "sourceLinks",
            "accountLabel",
            "teamName",
            "protocolReference",
            "rationale",
        ] {
            assert!(
                !seen.to_string().contains(&format!("\"{field}\":")),
                "private field {field}"
            );
        }
    }
}
