//! Cookie/CSRF extraction and request-header helpers shared by all suites.

use reqwest::header::{COOKIE, HeaderMap, HeaderValue};

/// Pull the `token` (JWT) and `csrf` cookie values out of a login response's
/// `set-cookie` headers. Missing cookies come back as empty strings.
pub fn extract_cookies(resp: &reqwest::Response) -> (String, String) {
    let mut jwt = String::new();
    let mut csrf = String::new();
    for cookie in resp.headers().get_all("set-cookie").iter() {
        if let Ok(val) = cookie.to_str() {
            if val.starts_with("token=") {
                jwt = val
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
    (jwt, csrf)
}

/// Build the cookie + `X-CSRF-Token` headers for an authenticated dashboard request.
pub fn auth_header(jwt: &str, csrf: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let cookie_val = format!("token={}; csrf={}", jwt, csrf);
    headers.insert(COOKIE, HeaderValue::from_str(&cookie_val).unwrap());
    headers.insert("X-CSRF-Token", HeaderValue::from_str(csrf).unwrap());
    headers
}
