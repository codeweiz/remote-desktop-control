use axum::{
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../web/dist/"]
struct WebAssets;

/// Serve embedded static files with SPA fallback.
///
/// 1. Try the exact URI path as a file in the embedded assets.
/// 2. If no file matches, serve `index.html` (SPA client-side routing).
/// 3. If `index.html` is also missing (dev build without web/dist), return 404.
pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Try exact file first
    if let Some(file) = WebAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(
                header::CACHE_CONTROL,
                if path.contains('.') && !path.ends_with(".html") {
                    "public, max-age=31536000, immutable" // fingerprinted assets
                } else {
                    "no-cache" // HTML pages
                },
            )
            .body(axum::body::Body::from(file.data.to_vec()))
            .unwrap()
            .into_response();
    }

    // SPA fallback: serve index.html for all non-file routes
    if let Some(index) = WebAssets::get("index.html") {
        return Html(String::from_utf8_lossy(&index.data).to_string()).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}
