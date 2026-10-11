use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use fsp_github_insights::client::GitHubClient;
use fsp_github_insights::collect::{Collector, Limits};
use fsp_github_insights::git::Git;
use fsp_github_insights::{InsightsState, router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().init();
    let host = env::var("FSP_GITHUB_INSIGHTS_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = env::var("FSP_GITHUB_INSIGHTS_PORT").unwrap_or_else(|_| "4030".into());
    // No token is required. An optional one only raises GitHub's per-IP anonymous limits.
    let token = env::var("GITHUB_TOKEN")
        .ok()
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty());
    // systemd's CacheDirectory= sets CACHE_DIRECTORY; clones are removed after each analysis.
    let scratch = env::var_os("FSP_GITHUB_INSIGHTS_SCRATCH")
        .or_else(|| env::var_os("CACHE_DIRECTORY"))
        .map_or_else(
            || env::temp_dir().join("fsp-github-insights"),
            PathBuf::from,
        );
    std::fs::create_dir_all(&scratch)?;
    let collector = Collector {
        client: Arc::new(GitHubClient::new(token)?),
        git: Arc::new(Git {
            base_url: "https://github.com".into(),
            scratch,
            clone_timeout: Duration::from_secs(180),
        }),
        limits: Limits::default(),
    };
    let state = Arc::new(InsightsState::new(Some(collector)));
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    tracing::info!(address = %listener.local_addr()?, "fsp-github-insights listening");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
