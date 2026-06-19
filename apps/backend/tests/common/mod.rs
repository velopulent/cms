//! Shared integration-test support for the `cms` backend.
//!
//! Hosts the real-server bootstrap (`TestServer`, `GrpcTestContext`) plus the
//! auth / fixture / client helpers that every test suite needs. This module is
//! `#[path]`-included by each suite's `main.rs`; since any given suite uses only
//! a subset of these helpers, `#![allow(dead_code)]` suppresses the per-binary
//! `dead_code` / `unused_imports` warnings that would otherwise fire for the
//! helpers and re-exports a given suite doesn't touch.
#![allow(dead_code, unused_imports)]

pub mod auth;
pub mod client;
pub mod fixtures;
pub mod grpc;
pub mod mcp;
mod server;
pub mod test_db;

pub use grpc::GrpcTestContext;
pub use server::TestServer;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;

/// Poll `addr` until a TCP connection succeeds or `timeout` elapses.
///
/// Replaces fixed `sleep`s after spawning a server: returns as soon as the
/// listener accepts, so it's both faster and race-free under load.
pub async fn wait_for_tcp(addr: SocketAddr, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if TcpStream::connect(addr).await.is_ok() {
            return;
        }
        if Instant::now() >= deadline {
            panic!("server at {addr} did not become ready within {timeout:?}");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
