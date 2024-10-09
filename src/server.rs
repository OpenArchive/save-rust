#![allow(unused)]
pub mod server {
    use actix_web::{web, App, Error as ActixError, HttpResponse, HttpServer, Responder};
    use actix_web::{get, patch, post, put};
    use anyhow::{Context, Result, anyhow};
    use base64_url;
    use futures::{future, lock};
    use once_cell::sync::OnceCell;
    use std::env;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;
    use save_dweb_backend::backend::Backend;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::fs;
    use thiserror::Error;
    use veilid_core::{
        vld0_generate_keypair, CryptoKey, TypedKey, VeilidUpdate, CRYPTO_KIND_VLD0, VALID_CRYPTO_KINDS
    };
    use crate::{log_debug, log_info, log_error};
    use crate::logging::android_log;
    use crate::constants::{self, TAG, VERSION};
    use crate::error::{AppError, AppResult};
    use crate::groups;
    use crate::repos;

    #[derive(Deserialize)]
    pub struct GroupPath {
        pub group_id: String,
    }

    #[derive(Deserialize)]
    pub struct GroupRepoPath {
        pub group_id: String,
        pub repo_id: String,
    }

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
            Backend::new(backend_path, 8080)
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

    pub async fn start(backend_base_directory: &str, server_socket_path: &str) -> anyhow::Result<()> {
        log_debug!(TAG, "start_server: Using socket path: {:?}", server_socket_path);

        if env::var("HOME").is_err() {
            env::set_var("HOME", backend_base_directory);
        }
        
        let backend_path = Path::new(backend_base_directory);

        BACKEND.get_or_init(|| init_backend(backend_path));
        
        {
            let mut backend = get_backend().await?;

            backend.start().await.context("Backend failed to start");
        }

        let web_server = HttpServer::new(move || {
            App::new()
            .app_data(web::JsonConfig::default().limit(4096))  // Limit payload size to 4kb
            .service(status)
            .service(
                web::scope("/api")
                    .service(groups::scope())
                    .service(repos::scope())
            )
        })
        .bind_uds(server_socket_path)?
        .run();

        log_info!(TAG, "Web server started");

        web_server.await.context("Failed to start server")
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