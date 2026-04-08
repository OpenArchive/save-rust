//! Desktop server binary for testing the save-dweb backend.
//!
//! Usage:
//!   cargo run --bin save-server [-- <base_dir>]
//!
//! The server listens on:
//!   - Unix socket: <base_dir>/save-server.sock
//!   - Optional HTTP: http://127.0.0.1:8080 when SAVE_ENABLE_TCP=1 and SAVE_API_TOKEN is set
//!
//! Set RUST_LOG to control log verbosity, e.g.:
//!   RUST_LOG=debug cargo run --bin save-server

use std::env;
use std::fs;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let base_dir = env::args().nth(1).unwrap_or_else(|| {
        let mut p = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        p.push("save-data");
        p.to_string_lossy().into_owned()
    });

    let socket_path = format!("{}/save-server.sock", &base_dir);

    // Ensure data directory exists
    fs::create_dir_all(&base_dir)?;

    // Remove stale socket file from a previous run
    let _ = fs::remove_file(&socket_path);

    println!("save-server v{}", env!("CARGO_PKG_VERSION"));
    println!("  Data directory: {base_dir}");
    println!("  Unix socket:    {socket_path}");
    println!("  HTTP:           disabled by default");
    if std::env::var("SAVE_ENABLE_TCP").ok().as_deref() == Some("1") {
        println!("  HTTP:           http://127.0.0.1:8080");
        println!("  Auth:           Authorization: Bearer $SAVE_API_TOKEN");
    }

    save::server::start(&base_dir, &socket_path).await
}
