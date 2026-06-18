//! Shared HTTP client constructor for tests.

/// A fresh `reqwest::Client`. Centralized so every call site stops rebuilding
/// `reqwest::Client::builder().build().unwrap()` inline.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder().build().unwrap()
}
