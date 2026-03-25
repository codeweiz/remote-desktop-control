use axum::{
    body::Body,
    http::{HeaderValue, Request, Response},
    middleware::Next,
};

/// Middleware that injects security-related response headers on every response.
///
/// Headers added:
/// - `X-Content-Type-Options: nosniff`
/// - `X-Frame-Options: DENY`
/// - `Referrer-Policy: strict-origin-when-cross-origin`
/// - `Permissions-Policy: camera=(), microphone=(), geolocation=()`
/// - `Content-Security-Policy` (basic policy)
pub async fn security_headers(
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let mut response = next.run(request).await;

    let headers = response.headers_mut();

    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; connect-src 'self' ws: wss:; style-src 'self' 'unsafe-inline'",
        ),
    );

    response
}
