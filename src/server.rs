#![allow(unused)]
pub mod server {
    use actix_web::{web, App, Error as ActixError, HttpResponse, HttpServer, Responder};
    use actix_web::{get, patch, post, put, delete};
    use anyhow::{Context, Result, anyhow};
    use base64_url;
    use futures::{future, lock};
    use num_cpus;
    use once_cell::sync::OnceCell;
    use std::{env, panic};
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;
    use save_dweb_backend::backend::Backend;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::cmp;
    use std::fs;
    use std::net::Ipv4Addr;
    use std::time::{Duration, Instant};
    use thiserror::Error;
    use veilid_core::{
        vld0_generate_keypair, CryptoKey, TypedKey, VeilidUpdate, CRYPTO_KIND_VLD0, VALID_CRYPTO_KINDS
    };
    use crate::actix_route_dumper::RouteDumper;
    use crate::{log_debug, log_info, log_error};
    use crate::logging::android_log;
    use crate::constants::{self, TAG, VERSION};
    use crate::error::{AppError, AppResult};
    use crate::models::SnowbirdGroup;
    use crate::groups;
    use crate::repos;

    #[derive(Error, Debug)]
    pub enum BackendError {
        #[error("Backend not initialized")]
        NotInitialized,

        #[error("Failed to initialize backend: {0}")]
        InitializationError(#[from] std::io::Error),
    }

    static BACKEND: OnceCell<Arc<TokioMutex<Backend>>> = OnceCell::new();

    pub async fn get_backend<'a>() -> Result<impl std::ops::DerefMut<Target = Backend> + 'a, anyhow::Error> {
        match BACKEND.get() {
            Some(backend) => Ok(backend.lock().await),
            None => Err(anyhow!("Backend not initialized"))
        }
    }

    fn init_backend(backend_path: &Path) -> Arc<TokioMutex<Backend>> {
        Arc::new(TokioMutex::new(
            Backend::new(backend_path)
                .expect("Failed to create Backend.")
        ))
    }

    #[get("/status")]
    async fn status() -> impl Responder {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "running",
            "version": *VERSION
        }))
    }

    #[derive(Deserialize)]
    struct JoinGroupRequest {
        uri: String
    }

    #[post("memberships")]
    async fn join_group(body: web::Json<JoinGroupRequest>) -> AppResult<impl Responder> {
        let join_request_data = body.into_inner();
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
        let worker_count = cmp::max(1, cmp::min(cpu_count / 2, 4));
        
        log_debug!(TAG, "Detected {} CPUs, using {} workers", cpu_count, worker_count);

        worker_count
    }

    pub async fn start(backend_base_directory: &str, server_socket_path: &str) -> anyhow::Result<()> {
        log_debug!(TAG, "start_server: Using socket path: {:?}", server_socket_path);

        let worker_count = get_optimal_worker_count();

        let start_instant = Instant::now();
        log_info!(TAG, "Starting server initialization...");

        let lan_address = Ipv4Addr::UNSPECIFIED; // 0.0.0.0
        let lan_port = 8080;

        panic::set_hook(Box::new(|panic_info| {
            log_error!(TAG, "Panic occurred: {:?}", panic_info);
        }));

        if env::var("HOME").is_err() {
            env::set_var("HOME", backend_base_directory);
        }
        
        let backend_path = Path::new(backend_base_directory);

        BACKEND.get_or_init(|| init_backend(backend_path));
        
        {
            let mut backend = get_backend().await?;

            backend.start().await.context("Backend failed to start");
        }

        log_perf("Backend started", start_instant.elapsed());

        let web_server = HttpServer::new(move || {
            let app_start = Instant::now();
            let app = App::new()
            .wrap(RouteDumper::new(actix_log))
            .service(status)
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
            Err(e) => log_error!(TAG, "Failed to shut down backend: {:?}", e)
        }

        Ok(())
    }
}