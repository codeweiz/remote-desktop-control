use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::CookieJar;

use crate::rate_limit::extract_client_ip;
use crate::state::AppState;

/// Cookie name used to persist the auth token on the client.
const TOKEN_COOKIE: &str = "rtb_token";

/// Authentication middleware.
///
/// Checks for a valid token in the following order:
/// 1. `Authorization: Bearer {token}` header
/// 2. `rtb_token` cookie
/// 3. `?token={token}` query parameter
///
/// When authenticated via query parameter the middleware:
/// - Sets an `rtb_token` cookie (HttpOnly, SameSite=Strict)
/// - Redirects to the same URL with the token parameter stripped
///
/// Returns `401 Unauthorized` if no valid credential is found.
pub async fn auth_middleware(
    State(state): State<AppState>,
    jar: CookieJar,
    request: Request<Body>,
    next: Next,
) -> Response {
    let ip = extract_client_ip(&request);

    // Check IP blocklist before spending time on token validation.
    if state.blocklist.is_banned(&ip) {
        return (StatusCode::FORBIDDEN, "Forbidden: IP banned").into_response();
    }

    let expected = state.token.read().await.clone();

    // 1. Check Authorization header
    if let Some(auth_header) = request.headers().get(header::AUTHORIZATION) {
        if let Ok(value) = auth_header.to_str() {
            if let Some(bearer_token) = value.strip_prefix("Bearer ") {
                if bearer_token == expected {
                    state.blocklist.record_success(&ip);
                    return next.run(request).await;
                }
            }
        }
    }

    // 2. Check cookie
    if let Some(cookie) = jar.get(TOKEN_COOKIE) {
        if cookie.value() == expected {
            state.blocklist.record_success(&ip);
            return next.run(request).await;
        }
    }

    // 3. Check query parameter
    if let Some(query) = request.uri().query() {
        for pair in query.split('&') {
            if let Some(token_value) = pair.strip_prefix("token=") {
                if token_value == expected {
                    state.blocklist.record_success(&ip);
                    // Build redirect URL without the token parameter
                    let clean_uri = strip_token_param(request.uri());
                    let cookie_header = format!(
                        "{}={}; HttpOnly; SameSite=Strict; Path=/",
                        TOKEN_COOKIE, expected
                    );
                    let mut response = Redirect::to(&clean_uri).into_response();
                    response.headers_mut().insert(
                        header::SET_COOKIE,
                        cookie_header.parse().unwrap(),
                    );
                    return response;
                }
            }
        }
    }

    // No valid credential found — record failure for blocklist tracking.
    state.blocklist.record_failure(&ip);
    (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
}

/// Remove the `token` query parameter from a URI, preserving other parameters.
fn strip_token_param(uri: &Uri) -> String {
    let path = uri.path();
    let query = match uri.query() {
        Some(q) => q,
        None => return path.to_string(),
    };

    let filtered: Vec<&str> = query
        .split('&')
        .filter(|pair| !pair.starts_with("token="))
        .collect();

    if filtered.is_empty() {
        path.to_string()
    } else {
        format!("{}?{}", path, filtered.join("&"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_token_only_param() {
        let uri: Uri = "/foo?token=abc".parse().unwrap();
        assert_eq!(strip_token_param(&uri), "/foo");
    }

    #[test]
    fn strip_token_with_other_params() {
        let uri: Uri = "/foo?bar=1&token=abc&baz=2".parse().unwrap();
        assert_eq!(strip_token_param(&uri), "/foo?bar=1&baz=2");
    }

    #[test]
    fn no_query_string() {
        let uri: Uri = "/foo".parse().unwrap();
        assert_eq!(strip_token_param(&uri), "/foo");
    }
}
