use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use fsp_application::{
    IntegrationError, SpeechAudioView, SpeechRecognitionGateway, SpeechSynthesisGateway,
    SpeechTranscriptView,
};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::json;

const MAX_AUDIO_BYTES: usize = 4 * 1024 * 1024;
const ACCEPTED_RECORDING_TYPES: [&str; 3] = ["audio/webm", "audio/mp4", "audio/ogg"];
const MAX_JSON_BYTES: usize = MAX_AUDIO_BYTES.div_ceil(3) * 4 + 64 * 1024;

pub struct GigaAmSpeechRecognition {
    client: Client,
    endpoint: Url,
}

impl GigaAmSpeechRecognition {
    pub fn new(endpoint: &str) -> Result<Self, IntegrationError> {
        Ok(Self {
            client: client_with_timeout(Duration::from_secs(45))?,
            endpoint: endpoint.parse().map_err(invalid_configuration)?,
        })
    }
}

#[derive(Deserialize)]
struct GigaAmTranscript {
    text: String,
    provider: String,
    duration_seconds: f32,
}

#[async_trait]
impl SpeechRecognitionGateway for GigaAmSpeechRecognition {
    async fn transcribe(
        &self,
        audio_base64: &str,
        mime_type: &str,
    ) -> Result<SpeechTranscriptView, IntegrationError> {
        if !ACCEPTED_RECORDING_TYPES.contains(&mime_type) {
            return Err(recognition_input_invalid());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(audio_base64)
            .map_err(|_| recognition_input_invalid())?;
        if bytes.is_empty() || bytes.len() > MAX_AUDIO_BYTES {
            return Err(recognition_input_invalid());
        }
        let transcript = self
            .client
            .post(self.endpoint.clone())
            .header(reqwest::header::CONTENT_TYPE, mime_type)
            .body(bytes)
            .send()
            .await
            .map_err(unavailable)?
            .error_for_status()
            .map_err(unavailable)?
            .json::<GigaAmTranscript>()
            .await
            .map_err(|_| invalid_response())?;
        if transcript.text.trim().is_empty() || transcript.duration_seconds <= 0.0 {
            return Err(invalid_response());
        }
        Ok(SpeechTranscriptView {
            text: transcript.text,
            provider: transcript.provider,
            duration_seconds: transcript.duration_seconds,
        })
    }
}

pub struct XaiSpeech {
    client: Client,
    endpoint: Url,
    api_key: String,
    voice: String,
}

impl XaiSpeech {
    pub fn new(endpoint: &str, api_key: String, voice: String) -> Result<Self, IntegrationError> {
        Ok(Self {
            client: client()?,
            endpoint: endpoint.parse().map_err(invalid_configuration)?,
            api_key,
            voice,
        })
    }
}

#[async_trait]
impl SpeechSynthesisGateway for XaiSpeech {
    async fn synthesize(&self, text: &str) -> Result<SpeechAudioView, IntegrationError> {
        validate_text(text)?;
        let response = self
            .client
            .post(self.endpoint.clone())
            .bearer_auth(&self.api_key)
            .json(&json!({
                "text": text,
                "voice_id": self.voice,
                "language": "ru",
                "output_format": {"codec": "mp3", "sample_rate": 24000, "bit_rate": 128000}
            }))
            .send()
            .await
            .map_err(unavailable)?
            .error_for_status()
            .map_err(unavailable)?;
        let mime_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("audio/mpeg")
            .split(';')
            .next()
            .unwrap_or("audio/mpeg")
            .to_owned();
        let bytes = bounded_body(response, MAX_AUDIO_BYTES).await?;
        if bytes.is_empty() || bytes.len() > MAX_AUDIO_BYTES {
            return Err(invalid_response());
        }
        Ok(SpeechAudioView {
            audio_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            mime_type,
            provider: "xai".into(),
            voice: self.voice.clone(),
        })
    }
}

pub struct YandexSpeech {
    client: Client,
    endpoint: Url,
    api_key: String,
    voice: String,
    role: Option<String>,
    model: Option<String>,
}

pub struct SileroSpeech {
    client: Client,
    endpoint: Url,
    voice: String,
}

impl SileroSpeech {
    pub fn new(endpoint: &str, voice: String) -> Result<Self, IntegrationError> {
        Ok(Self {
            client: client()?,
            endpoint: endpoint.parse().map_err(invalid_configuration)?,
            voice,
        })
    }
}

#[async_trait]
impl SpeechSynthesisGateway for SileroSpeech {
    async fn synthesize(&self, text: &str) -> Result<SpeechAudioView, IntegrationError> {
        validate_text(text)?;
        let response = self
            .client
            .post(self.endpoint.clone())
            .json(&json!({
                "text": text,
                "voice": self.voice,
                "sample_rate": 24_000
            }))
            .send()
            .await
            .map_err(unavailable)?
            .error_for_status()
            .map_err(unavailable)?;
        let bytes = bounded_body(response, MAX_AUDIO_BYTES).await?;
        if !valid_wav(&bytes) || bytes.len() > MAX_AUDIO_BYTES {
            return Err(invalid_response());
        }
        Ok(SpeechAudioView {
            audio_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            mime_type: "audio/wav".into(),
            provider: "silero_v5_5".into(),
            voice: self.voice.clone(),
        })
    }
}

impl YandexSpeech {
    pub fn new(
        endpoint: &str,
        api_key: String,
        voice: String,
        role: Option<String>,
        model: Option<String>,
    ) -> Result<Self, IntegrationError> {
        Ok(Self {
            client: client()?,
            endpoint: endpoint.parse().map_err(invalid_configuration)?,
            api_key,
            voice,
            role,
            model,
        })
    }
}

#[derive(Deserialize)]
struct YandexResponse {
    result: YandexResult,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct YandexResult {
    audio_chunk: YandexAudioChunk,
}

#[derive(Deserialize)]
struct YandexAudioChunk {
    data: String,
}

#[async_trait]
impl SpeechSynthesisGateway for YandexSpeech {
    async fn synthesize(&self, text: &str) -> Result<SpeechAudioView, IntegrationError> {
        validate_text(text)?;
        if text.chars().count() > 250 {
            return Err(invalid_input());
        }
        let mut hints = vec![json!({"voice": self.voice})];
        if let Some(role) = &self.role {
            hints.push(json!({"role": role}));
        }
        let mut body = json!({
            "text": text,
            "hints": hints,
            "outputAudioSpec": {"containerAudio": {"containerAudioType": "MP3"}},
            "loudnessNormalizationType": "LUFS"
        });
        if let Some(model) = &self.model {
            body["model"] = json!(model);
        }
        let response = self
            .client
            .post(self.endpoint.clone())
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Api-Key {}", self.api_key),
            )
            .json(&body)
            .send()
            .await
            .map_err(unavailable)?
            .error_for_status()
            .map_err(unavailable)?;
        let bytes = bounded_body(response, MAX_JSON_BYTES).await?;
        let response: YandexResponse =
            serde_json::from_slice(&bytes).map_err(|_| invalid_response())?;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&response.result.audio_chunk.data)
            .map_err(|_| invalid_response())?;
        if decoded.is_empty() || decoded.len() > MAX_AUDIO_BYTES {
            return Err(invalid_response());
        }
        Ok(SpeechAudioView {
            audio_base64: response.result.audio_chunk.data,
            mime_type: "audio/mpeg".into(),
            provider: "yandex_speechkit".into(),
            voice: self.voice.clone(),
        })
    }
}

async fn bounded_body(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, IntegrationError> {
    if response
        .content_length()
        .is_some_and(|size| size > limit as u64)
    {
        return Err(invalid_response());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(unavailable)? {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err(invalid_response());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn client() -> Result<Client, IntegrationError> {
    client_with_timeout(Duration::from_secs(20))
}

fn client_with_timeout(timeout: Duration) -> Result<Client, IntegrationError> {
    Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(invalid_configuration)
}

fn recognition_input_invalid() -> IntegrationError {
    IntegrationError {
        code: "speech_input_invalid",
        message: "Запись имеет неподдерживаемый формат или размер".into(),
    }
}

fn validate_text(text: &str) -> Result<(), IntegrationError> {
    if text.trim().is_empty() || text.chars().count() > 4_000 {
        return Err(invalid_input());
    }
    Ok(())
}

fn valid_wav(bytes: &[u8]) -> bool {
    bytes.len() >= 44 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE"
}

fn invalid_input() -> IntegrationError {
    IntegrationError {
        code: "speech_input_invalid",
        message: "Текст озвучивания имеет недопустимую длину".into(),
    }
}

fn invalid_configuration(_error: impl std::fmt::Display) -> IntegrationError {
    IntegrationError {
        code: "speech_not_configured",
        message: "Сервис озвучивания не настроен".into(),
    }
}

fn unavailable(_error: impl std::fmt::Display) -> IntegrationError {
    IntegrationError {
        code: "speech_unavailable",
        message: "Сервис озвучивания временно недоступен".into(),
    }
}

fn invalid_response() -> IntegrationError {
    IntegrationError {
        code: "speech_invalid_response",
        message: "Сервис озвучивания вернул некорректный ответ".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn response(headers: &str, body: &str) -> reqwest::Response {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let wire = format!("HTTP/1.1 200 OK\r\n{headers}\r\n{body}");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let received = stream.read(&mut request).await.unwrap();
            assert!(received > 0);
            stream.write_all(wire.as_bytes()).await.unwrap();
            // Deliberately keep unfinished responses open: the limit must stop the read.
            let _ = stream.read(&mut request).await;
        });
        client()
            .unwrap()
            .get(format!("http://{address}"))
            .send()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn rejects_advertised_oversize_before_waiting_for_body() {
        let response = response("Content-Length: 17\r\n", "").await;
        let error = tokio::time::timeout(Duration::from_secs(1), bounded_body(response, 16))
            .await
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, "speech_invalid_response");
    }

    #[tokio::test]
    async fn rejects_chunked_oversize_without_waiting_for_end_of_stream() {
        let response = response(
            "Transfer-Encoding: chunked\r\n",
            "8\r\n12345678\r\n8\r\n12345678\r\n1\r\nx\r\n",
        )
        .await;
        let error = tokio::time::timeout(Duration::from_secs(1), bounded_body(response, 16))
            .await
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, "speech_invalid_response");
    }

    #[tokio::test]
    async fn accepts_chunked_body_at_the_exact_limit() {
        let response = response(
            "Transfer-Encoding: chunked\r\n",
            "8\r\n12345678\r\n8\r\n12345678\r\n0\r\n\r\n",
        )
        .await;
        assert_eq!(
            bounded_body(response, 16).await.unwrap(),
            b"1234567812345678"
        );
    }

    #[test]
    fn rejects_empty_and_oversized_text() {
        assert_eq!(validate_text(" ").unwrap_err().code, "speech_input_invalid");
        assert_eq!(
            validate_text(&"я".repeat(4_001)).unwrap_err().code,
            "speech_input_invalid"
        );
        assert!(validate_text("Нормальный вопрос").is_ok());
    }

    #[test]
    fn yandex_rejects_text_above_provider_limit() {
        let adapter = YandexSpeech::new(
            "https://example.com",
            "key".into(),
            "marina".into(),
            None,
            None,
        )
        .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(
            runtime
                .block_on(adapter.synthesize(&"я".repeat(251)))
                .unwrap_err()
                .code,
            "speech_input_invalid"
        );
    }

    #[test]
    fn validates_wav_container_boundaries() {
        assert!(!valid_wav(&[]));
        assert!(!valid_wav(&[0; 44]));
        let mut wav = [0; 44];
        wav[..4].copy_from_slice(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        assert!(valid_wav(&wav));
    }
}
