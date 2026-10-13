use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = env::var("FSP_SIMULATOR_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = env::var("FSP_SIMULATOR_PORT").unwrap_or_else(|_| "4020".into());
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    axum::serve(listener, fsp_integration_fsp_synthetic::router())
        .with_graceful_shutdown(shutdown()?)
        .await?;
    Ok(())
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
