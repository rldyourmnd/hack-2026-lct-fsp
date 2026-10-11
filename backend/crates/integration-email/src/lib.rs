//! Transactional verification delivery. Credentials, recipients and codes are never logged.
use std::time::Duration;

use fsp_application::{IntegrationError, VerificationDelivery};
use serde_json::json;

const DEMO_EMAILS: [&str; 3] = [
    "alexey@example.com",
    "hr@sever-tech.example",
    "hr@second-company.example",
];

pub struct DemoDelivery(pub String);

#[async_trait::async_trait]
impl VerificationDelivery for DemoDelivery {
    fn code_for(&self) -> Result<String, IntegrationError> {
        Ok(self.0.clone())
    }
    async fn deliver(&self, _: &str, _: &str, _: &str) -> Result<(), IntegrationError> {
        Ok(())
    }
    fn contract_name(&self) -> &'static str {
        "mock_mailbox"
    }
}

pub struct HybridDelivery {
    demo: DemoDelivery,
    email: ResendDelivery,
}

impl HybridDelivery {
    #[must_use]
    pub fn new(demo_code: String, email: ResendDelivery) -> Self {
        Self {
            demo: DemoDelivery(demo_code),
            email,
        }
    }

    fn is_demo(email: &str) -> bool {
        DEMO_EMAILS.contains(&email)
    }
}

#[async_trait::async_trait]
impl VerificationDelivery for HybridDelivery {
    fn code_for(&self) -> Result<String, IntegrationError> {
        self.email.code_for()
    }

    fn code_for_email(&self, email: &str) -> Result<String, IntegrationError> {
        if Self::is_demo(email) {
            self.demo.code_for()
        } else {
            self.email.code_for()
        }
    }

    async fn deliver(
        &self,
        email: &str,
        code: &str,
        challenge_id: &str,
    ) -> Result<(), IntegrationError> {
        if Self::is_demo(email) {
            self.demo.deliver(email, code, challenge_id).await
        } else {
            self.email.deliver(email, code, challenge_id).await
        }
    }

    fn contract_name(&self) -> &'static str {
        "email"
    }

    fn contract_name_for_email(&self, email: &str) -> &'static str {
        if Self::is_demo(email) {
            self.demo.contract_name()
        } else {
            self.email.contract_name()
        }
    }
}

pub struct ResendDelivery {
    client: reqwest::Client,
    key: String,
    from: String,
    endpoint: String,
}

impl ResendDelivery {
    pub fn new(key: String, from: String) -> Result<Self, IntegrationError> {
        if !key.starts_with("re_")
            || key.trim() != key
            || key.chars().any(char::is_control)
            || from.len() > 254
            || from.split('@').count() != 2
            || !from
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"@._+-".contains(&c))
        {
            return Err(unavailable());
        }
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .connect_timeout(Duration::from_secs(3))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| unavailable())?,
            key,
            from,
            endpoint: "https://api.resend.com/emails".into(),
        })
    }
}

fn unavailable() -> IntegrationError {
    IntegrationError {
        code: "verification_delivery_failed",
        message: "Не удалось отправить письмо. Подождите минуту и попробуйте снова.".into(),
    }
}

#[async_trait::async_trait]
impl VerificationDelivery for ResendDelivery {
    fn code_for(&self) -> Result<String, IntegrationError> {
        // Rejection sampling avoids modulo bias. Every digit uses the operating-system CSPRNG.
        let mut code = String::with_capacity(6);
        while code.len() < 6 {
            let mut bytes = [0_u8; 16];
            getrandom::fill(&mut bytes).map_err(|_| unavailable())?;
            for byte in bytes {
                if byte < 250 {
                    code.push(char::from(b'0' + byte % 10));
                }
                if code.len() == 6 {
                    break;
                }
            }
        }
        Ok(code)
    }

    async fn deliver(
        &self,
        email: &str,
        code: &str,
        challenge_id: &str,
    ) -> Result<(), IntegrationError> {
        let mut response = self.client.post(&self.endpoint)
            .bearer_auth(&self.key)
            .header("Idempotency-Key", format!("verification/{challenge_id}"))
            .json(&json!({
                "from": format!("Платформа ФСП <{}>", self.from),
                "to": [email],
                "subject": "Код входа на платформу ФСП",
                "text": format!("Ваш код входа: {code}\n\nКод действует 15 минут и подходит только для запроса, в котором вы указали этот адрес. Никому не сообщайте код.\n\nЕсли вы не запрашивали вход, проигнорируйте письмо.\nПлатформа ФСП — https://nddev.link"),
                "html": format!("<!doctype html><html lang=\"ru\"><body><h1>Вход на платформу ФСП</h1><p>Ваш одноразовый код:</p><p style=\"font-size:32px;letter-spacing:6px\"><strong>{code}</strong></p><p>Код действует 15 минут и подходит только для текущего запроса входа. Никому не сообщайте код.</p><p>Если вы не запрашивали вход, проигнорируйте письмо.</p><p><a href=\"https://nddev.link\">Платформа ФСП</a></p></body></html>"),
                "tags": [{ "name": "challenge_id", "value": challenge_id }]
            })).send().await.map_err(|_| unavailable())?;
        if !response.status().is_success() {
            tracing::warn!(
                status = response.status().as_u16(),
                "verification provider rejected delivery"
            );
            return Err(unavailable());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
            if bytes.len() + chunk.len() > 16_384 {
                return Err(unavailable());
            }
            bytes.extend_from_slice(&chunk);
        }
        let result: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
        let id = result
            .get("id")
            .and_then(|id| id.as_str())
            .filter(|id| !id.is_empty() && id.len() <= 100)
            .ok_or_else(unavailable)?;
        tracing::info!(delivery_id = id, "verification email accepted by provider");
        Ok(())
    }

    fn contract_name(&self) -> &'static str {
        "email"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        http::{HeaderMap, StatusCode},
        routing::post,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn hybrid_delivery_limits_fixed_codes_to_seeded_accounts() {
        let email = ResendDelivery::new("re_test".into(), "login@nddev.link".into()).unwrap();
        let delivery = HybridDelivery::new("123456".into(), email);

        for address in DEMO_EMAILS {
            assert_eq!(delivery.contract_name_for_email(address), "mock_mailbox");
            assert_eq!(delivery.code_for_email(address).unwrap(), "123456");
        }
        assert_eq!(
            delivery.contract_name_for_email("candidate@auth.nddev.link"),
            "email"
        );
        assert_eq!(
            delivery
                .code_for_email("candidate@auth.nddev.link")
                .unwrap()
                .len(),
            6
        );
        assert_eq!(delivery.contract_name_for_email("new@example.com"), "email");
        assert_eq!(
            delivery.contract_name_for_email("Alexey@example.com"),
            "email"
        );
    }

    #[tokio::test]
    async fn sends_both_bodies_and_idempotency_without_following_redirects() {
        let seen = Arc::new(Mutex::new(None));
        let captured = seen.clone();
        let app = Router::new()
            .route(
                "/emails",
                post(
                    move |headers: HeaderMap, Json(body): Json<serde_json::Value>| {
                        let captured = captured.clone();
                        async move {
                            *captured.lock().unwrap() = Some((headers, body));
                            Json(json!({"id":"delivery-1"}))
                        }
                    },
                ),
            )
            .route(
                "/redirect",
                post(|| async { (StatusCode::TEMPORARY_REDIRECT, [("location", "/emails")]) }),
            )
            .route(
                "/invalid",
                post(|| async { Json(json!({"error": "not accepted"})) }),
            )
            .route("/oversized", post(|| async { "x".repeat(20_000) }))
            .route(
                "/rejected",
                post(|| async { (StatusCode::TOO_MANY_REQUESTS, "private provider details") }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut adapter =
            ResendDelivery::new("re_test".into(), "login@auth.example.com".into()).unwrap();
        adapter.endpoint = format!("{base}/emails");
        adapter
            .deliver("owner@example.com", "012345", "challenge-1")
            .await
            .unwrap();
        {
            let seen = seen.lock().unwrap();
            let (headers, body) = seen.as_ref().unwrap();
            assert_eq!(headers["idempotency-key"], "verification/challenge-1");
            assert_eq!(headers["authorization"], "Bearer re_test");
            assert_eq!(body["to"], json!(["owner@example.com"]));
            assert!(body["text"].as_str().unwrap().contains("012345"));
            assert!(body["html"].as_str().unwrap().contains("012345"));
            assert!(!body["subject"].as_str().unwrap().contains("012345"));
        }
        for path in ["redirect", "invalid", "oversized", "rejected"] {
            adapter.endpoint = format!("{base}/{path}");
            assert_eq!(
                adapter
                    .deliver("owner@example.com", "012345", "challenge-1")
                    .await
                    .unwrap_err()
                    .code,
                "verification_delivery_failed"
            );
        }
        server.abort();
    }

    #[test]
    fn random_codes_are_six_digits_and_configuration_rejects_header_injection() {
        let adapter =
            ResendDelivery::new("re_test".into(), "login@auth.example.com".into()).unwrap();
        let codes: std::collections::HashSet<_> =
            (0..100).map(|_| adapter.code_for().unwrap()).collect();
        assert!(codes.len() > 90);
        assert!(
            codes
                .iter()
                .all(|code| code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()))
        );
        assert!(
            ResendDelivery::new("re_test\r\n".into(), "login@auth.example.com".into()).is_err()
        );
        assert!(
            ResendDelivery::new(
                "re_test".into(),
                "login@auth.example.com\nBcc:other@example.com".into()
            )
            .is_err()
        );
    }
}
