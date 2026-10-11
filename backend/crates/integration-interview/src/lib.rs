use std::time::Duration;

use fsp_application::{
    IntegrationError, InterviewAnswerInput, InterviewGateway, InterviewStartInput, InterviewView,
    InterviewsView,
};
use reqwest::{Client, Response, StatusCode, Url};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub struct HttpInterviewGateway {
    client: Client,
    base: Url,
    token: String,
}

impl HttpInterviewGateway {
    pub fn new(base: &str, token: String) -> Result<Self, IntegrationError> {
        let base = Url::parse(base).map_err(|_| unavailable())?;
        if !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || base.path() != "/"
            || token.trim().is_empty()
        {
            return Err(unavailable());
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(50))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| unavailable())?;
        Ok(Self {
            client,
            base,
            token,
        })
    }

    async fn decode<T: DeserializeOwned>(response: Response) -> Result<T, IntegrationError> {
        let status = response.status();
        if !status.is_success() {
            let body = response.json::<ErrorBody>().await.ok();
            let code = match (status, body.as_ref().map(|body| body.code.as_str())) {
                (StatusCode::NOT_FOUND, Some("interview_not_found")) => "interview_not_found",
                (StatusCode::CONFLICT, Some("interview_conflict")) => "interview_conflict",
                (
                    StatusCode::UNPROCESSABLE_ENTITY | StatusCode::BAD_REQUEST,
                    Some("interview_input_invalid"),
                ) => "interview_input_invalid",
                (StatusCode::SERVICE_UNAVAILABLE, Some("interview_model_unavailable")) => {
                    "interview_model_unavailable"
                }
                (StatusCode::SERVICE_UNAVAILABLE, Some("interview_service_unavailable")) => {
                    "interview_service_unavailable"
                }
                (StatusCode::NOT_FOUND, _) => "interview_not_found",
                (StatusCode::CONFLICT, _) => "interview_conflict",
                (StatusCode::UNPROCESSABLE_ENTITY | StatusCode::BAD_REQUEST, _) => {
                    "interview_input_invalid"
                }
                _ => "interview_service_unavailable",
            };
            return Err(IntegrationError {
                code,
                message: body
                    .map(|body| body.message)
                    .unwrap_or_else(|| "Сервис собеседований временно недоступен".into()),
            });
        }
        response.json().await.map_err(|_| invalid_response())
    }

    fn url(&self, path: &str) -> Result<Url, IntegrationError> {
        self.base.join(path).map_err(|_| unavailable())
    }

    fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request.bearer_auth(&self.token)
    }
}

#[async_trait::async_trait]
impl InterviewGateway for HttpInterviewGateway {
    async fn list(&self, candidate_id: &str) -> Result<InterviewsView, IntegrationError> {
        Self::decode(
            self.authorize(
                self.client
                    .get(self.url(&format!("v1/candidates/{candidate_id}/interviews"))?),
            )
            .send()
            .await
            .map_err(|_| unavailable())?,
        )
        .await
    }

    async fn start(
        &self,
        candidate_id: &str,
        input: &InterviewStartInput,
    ) -> Result<InterviewView, IntegrationError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            candidate_id: &'a str,
            blueprint_id: &'a str,
            max_turns: Option<u8>,
        }
        Self::decode(
            self.authorize(self.client.post(self.url("v1/interviews")?))
                .json(&Request {
                    candidate_id,
                    blueprint_id: &input.blueprint_id,
                    max_turns: input.max_turns,
                })
                .send()
                .await
                .map_err(|_| unavailable())?,
        )
        .await
    }

    async fn get(&self, interview_id: &str) -> Result<InterviewView, IntegrationError> {
        Self::decode(
            self.authorize(
                self.client
                    .get(self.url(&format!("v1/interviews/{interview_id}"))?),
            )
            .send()
            .await
            .map_err(|_| unavailable())?,
        )
        .await
    }

    async fn answer(
        &self,
        interview_id: &str,
        input: &InterviewAnswerInput,
    ) -> Result<InterviewView, IntegrationError> {
        Self::decode(
            self.authorize(
                self.client
                    .post(self.url(&format!("v1/interviews/{interview_id}/answers"))?),
            )
            .json(input)
            .send()
            .await
            .map_err(|_| unavailable())?,
        )
        .await
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    code: String,
    message: String,
}

fn unavailable() -> IntegrationError {
    IntegrationError {
        code: "interview_service_unavailable",
        message: "Сервис собеседований временно недоступен".into(),
    }
}

fn invalid_response() -> IntegrationError {
    IntegrationError {
        code: "interview_service_unavailable",
        message: "Сервис собеседований вернул некорректный ответ".into(),
    }
}
