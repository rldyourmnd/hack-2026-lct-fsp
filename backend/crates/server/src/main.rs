//! Composition root: reads the configuration, wires adapters to the application and serves HTTP.

use std::env;
use std::error::Error;
use std::sync::Arc;

use fsp_application::{PlatformRepository, PlatformService, VerificationDelivery};
use fsp_http::HttpConfig;
use fsp_integration_bifrost::BifrostGateway;
use fsp_integration_email::{DemoDelivery, HybridDelivery, ResendDelivery};
use fsp_integration_fsp_synthetic::HttpFsp;
use fsp_integration_github::{GitHubApi, HttpInsights};
use fsp_integration_interview::HttpInterviewGateway;
use fsp_integration_speech::{GigaAmSpeechRecognition, SileroSpeech, XaiSpeech, YandexSpeech};
use fsp_storage_postgres::PostgresRepository;
use tokio::net::TcpListener;

const DEV_ORIGINS: &str = "http://127.0.0.1:5173,http://localhost:5173";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_current_span(true)
        .with_span_list(false)
        .init();

    match env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => {}
        [flag] if flag == "--check-pdf-runtime" => {
            fsp_integration_pdf::SandboxedPdf::default()
                .probe()
                .await
                .map_err(|error| format!("PDF adapter: {}", error.message))?;
            tracing::info!("sandboxed PDF extraction verified; database untouched");
            return Ok(());
        }
        _ => return Err("usage: fsp-career-api [--check-pdf-runtime]".into()),
    }

    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8080".into()).parse()?;
    let origins = env::var("FSP_ALLOWED_ORIGINS").unwrap_or_else(|_| DEV_ORIGINS.into());
    let allowed_origins = origins
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(String::from)
        .collect();

    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://fsp:fsp@127.0.0.1:5434/fsp".into());
    let mode = env::var("FSP_AUTH_MODE").unwrap_or_else(|_| "demo".into());
    let auth_key = if mode == "resend" {
        Some(nonempty_env("FSP_AUTH_KEY").ok_or("FSP_AUTH_KEY is required in resend mode")?)
    } else {
        None
    };
    let delivery: Arc<dyn VerificationDelivery> = match mode.as_str() {
        "demo" => {
            let code = env::var("FSP_DEMO_EMAIL_CODE").unwrap_or_else(|_| "123456".into());
            if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
                return Err("FSP_DEMO_EMAIL_CODE must contain six digits".into());
            }
            Arc::new(DemoDelivery(code))
        }
        "resend" => {
            let code = env::var("FSP_DEMO_EMAIL_CODE").unwrap_or_else(|_| "123456".into());
            if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
                return Err("FSP_DEMO_EMAIL_CODE must contain six digits".into());
            }
            Arc::new(HybridDelivery::new(
                code,
                ResendDelivery::new(
                    nonempty_env("RESEND_API_KEY").ok_or("RESEND_API_KEY is required")?,
                    nonempty_env("FSP_EMAIL_FROM").ok_or("FSP_EMAIL_FROM is required")?,
                )
                .map_err(|_| "Invalid Resend configuration")?,
            ))
        }
        _ => return Err("FSP_AUTH_MODE must be demo or resend".into()),
    };
    if let Some(key) = &auth_key
        && (key.len() != 64 || !key.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return Err("FSP_AUTH_KEY must be 32 random bytes encoded as 64 hex characters".into());
    }
    // `successfulGradeChangeCooldownDays` from meta.assessmentPolicy: 0-365,
    // default 90; out-of-range values fall back rather than crash the boot.
    let grade_cooldown_days = env::var("FSP_GRADE_CHANGE_COOLDOWN_DAYS")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|days| *days <= 365)
        .unwrap_or(90);
    // Validate fallible runtime configuration before any schema/data changes.
    let fsp = HttpFsp::new(
        &env::var("FSP_SIMULATOR_URL").unwrap_or_else(|_| "http://127.0.0.1:4020".into()),
    )
    .map_err(|error| std::io::Error::other(error.message))?;
    // Public GitHub analysis runs in the separate loopback insights service when configured.
    let insights = nonempty_env("FSP_GITHUB_INSIGHTS_URL")
        .map(|url| HttpInsights::new(&url))
        .transpose()
        .map_err(|error| format!("GitHub insights adapter: {}", error.message))?;
    let mut github_adapter = None;
    let mut llm_adapter = None;
    let mut pdf_adapter = None;
    if let (Some(client_id), Some(client_secret)) = (
        nonempty_env("GITHUB_CLIENT_ID"),
        nonempty_env("GITHUB_CLIENT_SECRET"),
    ) {
        let github = GitHubApi::new(client_id, client_secret)
            .map_err(|error| format!("GitHub adapter: {}", error.message))?;
        github_adapter = Some(Arc::new(github));
    }
    if let Some(chat_url) = nonempty_env("BIFROST_CHAT_URL") {
        let bifrost = BifrostGateway::new(
            chat_url,
            nonempty_env("BIFROST_API_KEY"),
            nonempty_env("BIFROST_MODEL").unwrap_or_else(|| "openai/gpt-5-mini".into()),
            // Advisory profile reviews use medium effort; `none` keeps the provider default.
            Some(nonempty_env("BIFROST_REASONING_EFFORT").unwrap_or_else(|| "medium".into()))
                .filter(|effort| effort != "none"),
        )
        .and_then(|gateway| {
            gateway.with_interactive_reasoning_effort(nonempty_env(
                "BIFROST_INTERACTIVE_REASONING_EFFORT",
            ))
        })
        .map_err(|error| format!("Bifrost adapter: {}", error.message))?;
        llm_adapter = Some(Arc::new(bifrost));
    }
    if env::var("FSP_PDF_IMPORT").is_ok_and(|value| value == "1") {
        let pdf = fsp_integration_pdf::SandboxedPdf::default();
        pdf.probe()
            .await
            .map_err(|error| format!("PDF adapter: {}", error.message))?;
        pdf_adapter = Some(Arc::new(pdf));
    }
    let speech_adapter: Option<Arc<dyn fsp_application::SpeechSynthesisGateway>> =
        match nonempty_env("FSP_TTS_PROVIDER").as_deref() {
            None | Some("browser") => None,
            Some("xai") => Some(Arc::new(
                XaiSpeech::new(
                    &env::var("XAI_TTS_URL").unwrap_or_else(|_| "https://api.x.ai/v1/tts".into()),
                    nonempty_env("XAI_API_KEY").ok_or("XAI_API_KEY is required for xai TTS")?,
                    env::var("XAI_TTS_VOICE").unwrap_or_else(|_| "eve".into()),
                )
                .map_err(|error| format!("xAI speech adapter: {}", error.message))?,
            )),
            Some("yandex") => Some(Arc::new(
                YandexSpeech::new(
                    &env::var("YANDEX_TTS_URL").unwrap_or_else(|_| {
                        "https://tts.api.cloud.yandex.net/tts/v3/utteranceSynthesis".into()
                    }),
                    nonempty_env("YANDEX_SPEECHKIT_API_KEY")
                        .ok_or("YANDEX_SPEECHKIT_API_KEY is required for yandex TTS")?,
                    env::var("YANDEX_TTS_VOICE").unwrap_or_else(|_| "marina".into()),
                    nonempty_env("YANDEX_TTS_ROLE"),
                    nonempty_env("YANDEX_TTS_MODEL"),
                )
                .map_err(|error| format!("Yandex speech adapter: {}", error.message))?,
            )),
            Some("silero") => Some(Arc::new(
                SileroSpeech::new(
                    &env::var("SILERO_TTS_URL")
                        .unwrap_or_else(|_| "http://127.0.0.1:4050/synthesize".into()),
                    env::var("SILERO_TTS_VOICE").unwrap_or_else(|_| "eugene".into()),
                )
                .map_err(|error| format!("Silero speech adapter: {}", error.message))?,
            )),
            Some(_) => {
                return Err("FSP_TTS_PROVIDER must be browser, silero, xai or yandex".into());
            }
        };
    let transcription_adapter: Option<Arc<dyn fsp_application::SpeechRecognitionGateway>> =
        match nonempty_env("FSP_STT_PROVIDER").as_deref() {
            None => None,
            Some("gigaam") => Some(Arc::new(
                GigaAmSpeechRecognition::new(
                    &env::var("GIGAAM_STT_URL")
                        .unwrap_or_else(|_| "http://127.0.0.1:4060/transcribe".into()),
                )
                .map_err(|error| format!("GigaAM speech adapter: {}", error.message))?,
            )),
            Some(_) => return Err("FSP_STT_PROVIDER must be gigaam".into()),
        };
    let listener = TcpListener::bind((host.as_str(), port)).await?;
    let repository = Arc::new(
        PostgresRepository::connect(&database_url)
            .await
            .map_err(|error| format!("{error:?}"))?,
    );
    let mut platform = PlatformService::new(repository.clone(), delivery)
        .with_grade_cooldown_days(grade_cooldown_days)
        .with_fsp(Arc::new(fsp));
    if let Some(key) = auth_key {
        platform = platform.with_auth_key(&key)?;
    }
    if let Some(github) = github_adapter {
        platform = platform.with_github(github);
    }
    if let Some(insights) = insights {
        platform = platform.with_github_insights(Arc::new(insights));
    }
    if let Some(llm) = llm_adapter {
        platform = platform.with_llm(llm);
    }
    if let Some(pdf) = pdf_adapter {
        platform = platform.with_pdf(pdf);
    }
    if let Some(speech) = speech_adapter {
        platform = platform.with_speech(speech);
    }
    if let Some(transcription) = transcription_adapter {
        platform = platform.with_transcription(transcription);
    }
    match (
        nonempty_env("FSP_INTERVIEW_URL"),
        nonempty_env("INTERVIEW_API_TOKEN"),
    ) {
        (Some(url), Some(token)) => {
            platform = platform.with_interviews(Arc::new(
                HttpInterviewGateway::new(&url, token)
                    .map_err(|error| format!("Interview adapter: {}", error.message))?,
            ));
        }
        (None, None) => {}
        _ => {
            return Err(
                "FSP_INTERVIEW_URL and INTERVIEW_API_TOKEN must be configured together".into(),
            );
        }
    }
    let service = Arc::new(platform);
    service.initialize().await.map_err(|error| error.message)?;
    // Expired challenges, sessions and stale idempotency markers are workload
    // the service layer cannot schedule on its own; sweep them once a minute.
    tokio::spawn(sweep_expired(repository.clone()));
    let secure_cookie = env::var("FSP_SECURE_COOKIE").is_ok_and(|value| value == "1");
    let public_web_url =
        env::var("FSP_PUBLIC_WEB_URL").unwrap_or_else(|_| "http://127.0.0.1:5173".into());
    let github_callback_url = env::var("GITHUB_CALLBACK_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080/api/v1/integrations/github/callback".into());
    let app = fsp_http::router(
        HttpConfig {
            allowed_origins,
            secure_cookie,
            public_web_url,
            github_callback_url,
            static_dir: nonempty_env("FSP_STATIC_DIR").map(Into::into),
        },
        service,
    );
    tracing::info!(address = %listener.local_addr()?, "fsp-career-api listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown()?)
        .await?;
    Ok(())
}

fn nonempty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

async fn sweep_expired(repository: Arc<PostgresRepository>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    loop {
        interval.tick().await;
        let at = chrono::Utc::now().to_rfc3339();
        if let Err(error) = repository.sweep_expired(&at).await {
            tracing::warn!(error = ?error, "expired-record sweep failed");
        }
    }
}

// Register before axum spawns its shutdown task, so setup failures exit main with an error.
fn shutdown() -> std::io::Result<impl std::future::Future<Output = ()>> {
    #[cfg(unix)]
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    #[cfg(windows)]
    let mut interrupt = tokio::signal::windows::ctrl_c()?;
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    Ok(async move {
        #[cfg(unix)]
        tokio::select! {
            _ = interrupt.recv() => {}
            _ = terminate.recv() => {}
        }
        #[cfg(windows)]
        {
            interrupt.recv().await;
        }
    })
}
