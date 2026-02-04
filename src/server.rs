#![allow(unused)]
use crate::constants::{self, TAG, VERSION};
use crate::error::{AppError, AppResult};
use crate::groups;
use crate::logging::android_log;
use crate::repos;
use crate::{log_debug, log_error, log_info};
use actix_web::{get, post};
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use anyhow::{anyhow, Context, Result};
use num_cpus;
use once_cell::sync::OnceCell;
use save_dweb_backend::backend::Backend;
use serde::Deserialize;
use serde_json::json;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};
use std::path::Path;
use std::sync::Arc;
use std::{env, panic};
use thiserror::Error;

#[cfg(test)]
use std::sync::RwLock;
use veilid_core::VeilidUpdate;
use crate::actix_route_dumper::RouteDumper;
use crate::models::SnowbirdGroup;

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("Backend not initialized")]
    NotInitialized,

    #[error("Failed to initialize backend: {0}")]
    InitializationError(#[from] std::io::Error),
}

// Production: use OnceCell (efficient, set-once)
#[cfg(not(test))]
pub static BACKEND: OnceCell<Arc<Backend>> = OnceCell::new();

// Tests: use RwLock (resettable between tests)
#[cfg(test)]
pub static BACKEND: RwLock<Option<Arc<Backend>>> = RwLock::new(None);

pub async fn get_backend() -> Result<Arc<Backend>, anyhow::Error> {
    #[cfg(not(test))]
    {
        match BACKEND.get() {
            Some(backend) => Ok(Arc::clone(backend)),
            None => Err(anyhow!("Backend not initialized")),
        }
    }
    #[cfg(test)]
    {
        let backend_lock = BACKEND.read().map_err(|e| anyhow!("Failed to read backend lock: {e}"))?;
        match backend_lock.as_ref() {
            Some(backend) => Ok(Arc::clone(backend)),
            None => Err(anyhow!("Backend not initialized")),
        }
    }
}

#[cfg(test)]
pub fn set_backend(backend: Arc<Backend>) -> Result<()> {
    let mut backend_lock = BACKEND.write().map_err(|e| anyhow!("Failed to write backend lock: {e}"))?;
    *backend_lock = Some(backend);
    Ok(())
}

#[cfg(test)]
pub fn clear_backend() -> Result<()> {
    let mut backend_lock = BACKEND.write().map_err(|e| anyhow!("Failed to write backend lock: {e}"))?;
    *backend_lock = None;
    Ok(())
}

/// Ensure backend is initialized before proceeding with operations
pub async fn ensure_backend_ready() -> AppResult<()> {
    let backend = get_backend().await?;
    // Check if iroh_blobs is initialized by trying to get it
    // This will fail gracefully if not initialized
    match backend.get_iroh_blobs().await {
        Some(_) => Ok(()),
        None => Err(crate::error::AppError::from(anyhow!(
            "Backend not ready. Veilid Iroh Blobs API not initialized. Initialization may still be in progress."
        ))),
    }
}

pub fn init_backend(backend_path: &Path) -> Arc<Backend> {
    Arc::new(Backend::new(backend_path).expect("Failed to create Backend."))
}

#[get("/status")]
async fn status() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "running",
        "version": *VERSION
    }))
}

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "OK"
    }))
}

#[get("/health/ready")]
async fn health_ready() -> AppResult<impl Responder> {
    let backend = get_backend().await?;

    if !backend.is_initialized().await {
        return Err(crate::error::AppError::from(anyhow!(
            "Backend not ready. Initialization in progress."
        )));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "ready",
        "initialized": true
    })))
}

#[derive(Deserialize)]
struct JoinGroupRequest {
    uri: String
}

#[post("memberships")]
async fn join_group(body: web::Json<JoinGroupRequest>) -> AppResult<impl Responder> {
    let join_request_data = body.into_inner();
    
    // Ensure backend is fully initialized before proceeding
    ensure_backend_ready().await?;
    
    let backend = get_backend().await?;
    let boxed_group = backend.join_from_url(&join_request_data.uri).await?;
    let snowbird_group: SnowbirdGroup = boxed_group.as_ref().into();

    Ok(HttpResponse::Ok().json(json!({ "group" : snowbird_group })))
}

fn actix_log(message: &str) {
    log_debug!(TAG, "Actix log: {}", message);
}

fn log_perf(message: &str, duration: Duration) {
    let total_ms = duration.as_millis();
    let rounded_tenths = (total_ms as f64 / 100.0).round() / 10.0;
    log_info!(TAG, "{} after {:.1} s", message, rounded_tenths);
}

fn get_optimal_worker_count() -> usize {
    let cpu_count = num_cpus::get();
    log_debug!(TAG, "Detected {} CPUs", cpu_count);

    // Allow override via environment variable for testing
    if let Ok(worker_count_str) = env::var("SAVE_WORKER_COUNT") {
        if let Ok(worker_count) = worker_count_str.parse::<usize>() {
            log_info!(TAG, "Using SAVE_WORKER_COUNT={} (override)", worker_count);
            return worker_count;
        } else {
            log_error!(TAG, "Invalid SAVE_WORKER_COUNT value: {}, using default", worker_count_str);
        }
    }

    // Default: Backend has internal mutex; multiple workers help with
    // CPU-bound work (JSON parsing/serialization) and concurrent request handling.
    // Original optimization attempt: cmp::max(1, cmp::min(cpu_count / 2, 4))
    1
}

pub async fn start(backend_base_directory: &str, server_socket_path: &str) -> anyhow::Result<()> {
    log_debug!(TAG, "start_server: Using socket path: {:?}", server_socket_path);

    let worker_count = get_optimal_worker_count();

    let start_instant = Instant::now();
    log_info!(TAG, "Starting server initialization...");

    let lan_address = Ipv4Addr::LOCALHOST; // 127.0.0.1
    let lan_port = 8080;

    panic::set_hook(Box::new(|panic_info| {
        log_error!(TAG, "Panic occurred: {:?}", panic_info);
    }));

    if env::var("HOME").is_err() {
        env::set_var("HOME", backend_base_directory);
    }

    let backend_path = Path::new(backend_base_directory);

    #[cfg(not(test))]
    BACKEND.get_or_init(|| init_backend(backend_path));

    #[cfg(test)]
    {
        let _ = set_backend(init_backend(backend_path));
    }

    // Start backend initialization in the background so the HTTP server can come up immediately.
    let backend_arc = {
        #[cfg(not(test))]
        {
            BACKEND.get().cloned()
        }
        #[cfg(test)]
        {
            BACKEND
                .read()
                .ok()
                .and_then(|backend| backend.as_ref().cloned())
        }
    };

    if let Some(backend_arc) = backend_arc {
        tokio::spawn(async move {
            if let Err(e) = backend_arc.start().await {
                log_error!(TAG, "Backend failed to start: {:?}", e);
            }
        });
    } else {
        log_error!(TAG, "Backend not initialized; cannot start in background");
    }

    log_perf("Backend init scheduled", start_instant.elapsed());

    let web_server = HttpServer::new(move || {
        let app_start = Instant::now();
        let app = App::new()
        .wrap(RouteDumper::new(actix_log))
        .service(status)
        .service(health)
        .service(health_ready)
        .service(
            web::scope("/api")
                .service(join_group)
                .service(groups::scope())
        );
        log_perf("Web server app created", app_start.elapsed());
        app
    })
    .bind_uds(server_socket_path)?
    .bind((lan_address, lan_port))?
    .disable_signals()
    .workers(worker_count);

    log_perf("Web server initialized", start_instant.elapsed());
    log_info!(TAG, "Starting web server...");
    
    let server_future = web_server.run();
    log_perf("Web server started", start_instant.elapsed());

    server_future.await.context("Failed to start server")
}

pub async fn stop() -> anyhow::Result<()> {
    let mut backend = get_backend().await?;

    match backend.stop().await {
        Ok(_) => log_debug!(TAG, "Backend shut down successfully."),
        Err(e) => log_error!(TAG, "Failed to shut down backend: {:?}", e),
    }

    Ok(())
}
