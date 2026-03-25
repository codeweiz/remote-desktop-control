//! Structured access logging and tracing setup for RTB server.
//!
//! Provides:
//! - `setup_logging()` — initialize tracing subscriber with console + file output
//! - `access_log_layer()` — Axum middleware layer for per-request access logging

use std::path::Path;
use std::time::Instant;

use axum::{
    body::Body,
    http::{Request, Response},
    middleware::Next,
};
use rtb_core::config::Config;
use tracing_appender::rolling;
use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

/// Initialize the global tracing subscriber.
///
/// Sets up two output targets:
/// - **Console**: human-readable logs filtered by `RUST_LOG` env or `config.logging.level`
/// - **File**: access logs written to `~/.rtb/logs/access.jsonl` using a rolling appender
///
/// This must be called exactly once, before any tracing macros are used.
pub fn setup_logging(config: &Config) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.logging.level));

    // Resolve access log directory and file name from config
    let access_log_path = &config.logging.access_log;

    // Try to set up file appender for access logs
    let file_layer = if !access_log_path.is_empty() {
        let log_path = Path::new(access_log_path);
        let log_dir = log_path
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let log_filename = log_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("access.jsonl");

        // Create log directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(log_dir) {
            eprintln!("Warning: could not create log directory {}: {}", log_dir.display(), e);
            None
        } else {
            let file_appender = rolling::never(log_dir, log_filename);
            Some(
                fmt::layer()
                    .json()
                    .with_target(false)
                    .with_level(false)
                    .with_writer(file_appender)
                    .with_filter(EnvFilter::new("rtb_server::logging=info")),
            )
        }
    } else {
        None
    };

    // Console layer for application logs
    let console_layer = fmt::layer()
        .with_target(false)
        .with_filter(filter);

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    Ok(())
}

/// Axum middleware that logs structured access information for each request.
///
/// Logs: timestamp, client IP, HTTP method, path, response status, latency (ms).
pub async fn access_log_middleware(
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let start = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    // Try to extract client IP from ConnectInfo or forwarded headers
    let ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(',').next().unwrap_or("unknown").trim().to_string())
        .or_else(|| {
            request
                .headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    let response = next.run(request).await;

    let elapsed_ms = start.elapsed().as_millis();
    let status = response.status().as_u16();

    // Structured access log — this gets captured by the file layer
    tracing::info!(
        target: "rtb_server::logging",
        ip = %ip,
        method = %method,
        path = %path,
        status = status,
        ms = elapsed_ms,
        "access"
    );

    response
}
