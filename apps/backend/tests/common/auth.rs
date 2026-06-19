//! Cookie/CSRF extraction and request-header helpers shared by all suites.

use reqwest::header::{COOKIE, HeaderMap, HeaderValue};

/// Pull the `token` (opaque session token) and `csrf` cookie values out of a login response's
/// `set-cookie` headers. Missing cookies come back as empty strings.
pub fn extract_cookies(resp: &reqwest::Response) -> (String, String) {
    let mut token = String::new();
    let mut csrf = String::new();
    for cookie in resp.headers().get_all("set-cookie").iter() {
        if let Ok(val) = cookie.to_str() {
            if val.starts_with("token=") {
                token = val
                    .split(';')
                    .next()
                    .and_then(|c| c.strip_prefix("token="))
                    .unwrap_or("")
                    .to_string();
            }
            if val.starts_with("csrf=") {
                csrf = val
                    .split(';')
                    .next()
                    .and_then(|c| c.strip_prefix("csrf="))
                    .unwrap_or("")
                    .to_string();
            }
        }
    }
    (token, csrf)
}

/// Build the cookie + `X-CSRF-Token` headers for an authenticated dashboard request.
pub fn auth_header(token: &str, csrf: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let cookie_val = format!("token={}; csrf={}", token, csrf);
    headers.insert(COOKIE, HeaderValue::from_str(&cookie_val).unwrap());
    headers.insert("X-CSRF-Token", HeaderValue::from_str(csrf).unwrap());
    headers
}
