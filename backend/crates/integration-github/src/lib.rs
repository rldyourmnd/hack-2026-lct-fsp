//! GitHub adapters: OAuth identity (an optional ownership proof) and the client of the loopback
//! insights service, which performs all public-activity analysis.

use std::time::Duration;

use async_trait::async_trait;
use fsp_application::{
    GitHubGateway, GitHubInsights, GitHubInsightsGateway, InsightsStatus, IntegrationError,
};
use fsp_domain::GitHubReport;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};

pub struct GitHubApi {
    client: Client,
    api_base: String,
    client_id: String,
    client_secret: String,
}

impl GitHubApi {
    pub fn new(client_id: String, client_secret: String) -> Result<Self, IntegrationError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("vygotsky-fsp/0.1")
            .build()
            .map_err(integration)?;
        Ok(Self {
            client,
            api_base: "https://api.github.com".into(),
            client_id,
            client_secret,
        })
    }
}

#[async_trait]
impl GitHubGateway for GitHubApi {
    fn authorization_url(
        &self,
        state: &str,
        redirect_uri: &str,
    ) -> Result<String, IntegrationError> {
        let mut url =
            Url::parse("https://github.com/login/oauth/authorize").map_err(integration)?;
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", "read:user")
            .append_pair("state", state);
        Ok(url.into())
    }

    async fn exchange_code(&self, code: &str) -> Result<String, IntegrationError> {
        let response = self
            .client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .json(&TokenRequest {
                client_id: &self.client_id,
                client_secret: &self.client_secret,
                code,
            })
            .send()
            .await
            .map_err(integration)?
            .error_for_status()
            .map_err(integration)?
            .json::<TokenResponse>()
            .await
            .map_err(integration)?;
        response.access_token.ok_or_else(|| IntegrationError {
            code: "github_oauth_failed",
            message: "GitHub не выдал токен доступа".into(),
        })
    }

    async fn authenticated_login(&self, token: &str) -> Result<String, IntegrationError> {
        let user = self
            .client
            .get(format!("{}/user", self.api_base))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(integration)?
            .error_for_status()
            .map_err(integration)?
            .json::<ApiUser>()
            .await
            .map_err(integration)?;
        Ok(user.login)
    }
}

/// Client of the `fsp-github-insights` loopback service.
pub struct HttpInsights {
    client: Client,
    base: String,
}

impl HttpInsights {
    pub fn new(base: &str) -> Result<Self, IntegrationError> {
        let base = Url::parse(base).map_err(integration)?;
        if !matches!(base.scheme(), "http" | "https") {
            return Err(unavailable());
        }
        Ok(Self {
            // Analyses run in the background; each call only queues or reads status.
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .connect_timeout(Duration::from_secs(2))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(integration)?,
            base: base.as_str().trim_end_matches('/').to_owned(),
        })
    }
}

#[async_trait]
impl GitHubInsightsGateway for HttpInsights {
    async fn submit(
        &self,
        login: &str,
        ownership_code: Option<&str>,
        refresh: bool,
    ) -> Result<InsightsStatus, IntegrationError> {
        let response = self
            .client
            .post(format!("{}/v1/analyses", self.base))
            .json(&SubmitRequest {
                login,
                ownership_code,
                refresh,
            })
            .send()
            .await
            .map_err(integration)?;
        read_status(response).await?.ok_or_else(unavailable)
    }

    async fn status(
        &self,
        login: &str,
        ownership_code: Option<&str>,
    ) -> Result<Option<InsightsStatus>, IntegrationError> {
        let mut request = self
            .client
            .get(format!("{}/v1/analyses/{login}", self.base));
        if let Some(code) = ownership_code {
            request = request.query(&[("ownershipCode", code)]);
        }
        read_status(request.send().await.map_err(integration)?).await
    }
}

async fn read_status(
    response: reqwest::Response,
) -> Result<Option<InsightsStatus>, IntegrationError> {
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        let code = response
            .json::<ServiceError>()
            .await
            .map(|error| error.code)
            .unwrap_or_default();
        return Err(service_error(&code));
    }
    let body = response
        .json::<AnalysisResponse>()
        .await
        .map_err(integration)?;
    Ok(Some(match body.state.as_str() {
        "done" => InsightsStatus::Done(Box::new(GitHubInsights {
            report: body.report.ok_or_else(unavailable)?,
            ownership_code_found: body.ownership_code_found,
        })),
        "failed" => InsightsStatus::Failed(service_error(
            body.error.as_ref().map_or("", |error| error.code.as_str()),
        )),
        state => InsightsStatus::Pending {
            queued: state == "queued",
            stage: body.stage,
            progress: body.progress,
        },
    }))
}

/// Stable public errors for the service's codes; unknown codes and details are never reflected.
fn service_error(code: &str) -> IntegrationError {
    match code {
        "github_login_invalid" => IntegrationError {
            code: "github_login_invalid",
            message: "Укажите имя пользователя GitHub или ссылку на профиль".into(),
        },
        "github_account_not_found" => IntegrationError {
            code: "github_account_not_found",
            message: "Пользователь GitHub не найден".into(),
        },
        "github_rate_limited" => IntegrationError {
            code: "github_rate_limited",
            message: "GitHub временно ограничил запросы, повторите позже".into(),
        },
        "github_not_configured" => IntegrationError {
            code: "github_not_configured",
            message: "Подключение GitHub временно недоступно".into(),
        },
        _ => unavailable(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitRequest<'a> {
    login: &'a str,
    ownership_code: Option<&'a str>,
    refresh: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisResponse {
    state: String,
    stage: String,
    progress: u8,
    report: Option<GitHubReport>,
    ownership_code_found: Option<bool>,
    error: Option<ServiceError>,
}

#[derive(Deserialize)]
struct ServiceError {
    code: String,
}

#[derive(Deserialize)]
struct ApiUser {
    login: String,
}

#[derive(Serialize)]
struct TokenRequest<'a> {
    client_id: &'a str,
    client_secret: &'a str,
    code: &'a str,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
}

fn unavailable() -> IntegrationError {
    IntegrationError {
        code: "github_unavailable",
        message: "GitHub временно недоступен".into(),
    }
}

fn integration(_error: impl std::fmt::Display) -> IntegrationError {
    unavailable()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn oauth_identity_is_read_from_the_token() {
        use axum::{Router, routing::get};
        let app = Router::new().route(
            "/user",
            get(|| async { axum::Json(serde_json::json!({ "login": "Owner" })) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut api = GitHubApi::new("test".into(), "test".into()).unwrap();
        api.api_base = format!("http://{address}");
        assert_eq!(
            api.authenticated_login("fixture-token").await.unwrap(),
            "Owner"
        );
        server.abort();
    }

    #[tokio::test]
    async fn insights_states_and_errors_keep_their_meaning_without_upstream_details() {
        use axum::{
            Router,
            http::StatusCode,
            routing::{get, post},
        };
        let app = Router::new()
            .route(
                "/v1/analyses",
                post(|axum::Json(body): axum::Json<serde_json::Value>| async move {
                    match body["login"].as_str() {
                        Some("tokenless") => (
                            StatusCode::SERVICE_UNAVAILABLE,
                            r#"{"code":"github_not_configured","message":"x"}"#.to_owned(),
                        ),
                        Some("broken") => (StatusCode::BAD_GATEWAY, "secret upstream detail".into()),
                        _ => (
                            StatusCode::OK,
                            r#"{"login":"dev","state":"running","stage":"Поиск коммитов","progress":8,"report":null,"ownershipCodeFound":null,"error":null,"retryAfterSeconds":null}"#.into(),
                        ),
                    }
                }),
            )
            .route(
                "/v1/analyses/{login}",
                get(|axum::extract::Path(login): axum::extract::Path<String>| async move {
                    match login.as_str() {
                        "missing" => (StatusCode::NOT_FOUND, String::new()),
                        _ => (
                            StatusCode::OK,
                            r#"{"login":"x","state":"failed","stage":"Ошибка","progress":0,"report":null,"ownershipCodeFound":null,"error":{"code":"github_account_not_found","message":"secret"},"retryAfterSeconds":null}"#.into(),
                        ),
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let insights = HttpInsights::new(&format!("http://{address}")).unwrap();
        assert_eq!(
            insights.submit("dev", None, false).await.unwrap(),
            InsightsStatus::Pending {
                queued: false,
                stage: "Поиск коммитов".into(),
                progress: 8
            }
        );
        assert_eq!(
            insights
                .submit("tokenless", None, false)
                .await
                .unwrap_err()
                .code,
            "github_not_configured"
        );
        let broken = insights.submit("broken", None, false).await.unwrap_err();
        assert_eq!(broken.code, "github_unavailable");
        assert!(!broken.message.contains("secret"));
        assert_eq!(insights.status("missing", None).await.unwrap(), None);
        let InsightsStatus::Failed(error) = insights
            .status("x", Some("fsp-0123456789"))
            .await
            .unwrap()
            .unwrap()
        else {
            panic!("expected a failed analysis");
        };
        assert_eq!(error.code, "github_account_not_found");
        assert!(!error.message.contains("secret"));
        server.abort();
    }

    #[test]
    fn upstream_error_details_are_not_public() {
        let error = integration("https://user:private@example.invalid/?access_token=secret");
        assert_eq!(error.message, "GitHub временно недоступен");
    }

    #[test]
    fn authorization_url_keeps_state_and_callback_bound_to_the_request() {
        let gateway = GitHubApi::new("client-id".into(), "secret".into()).unwrap();
        let url = gateway
            .authorization_url(
                "state with spaces",
                "https://example.test/oauth/callback?source=github",
            )
            .unwrap();
        let parsed = Url::parse(&url).unwrap();
        let query = parsed
            .query_pairs()
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(
            query.get("client_id").map(|value| value.as_ref()),
            Some("client-id")
        );
        assert_eq!(
            query.get("state").map(|value| value.as_ref()),
            Some("state with spaces")
        );
        assert_eq!(
            query.get("redirect_uri").map(|value| value.as_ref()),
            Some("https://example.test/oauth/callback?source=github")
        );
    }
}
