#![allow(unused)]
pub mod server {
    use actix_web::{web, App, Error as ActixError, HttpResponse, HttpServer, Responder};
    use actix_web::{get, patch, post, put};
    use anyhow::{Context, Result, anyhow};
    use base64_url;
    use eyre::Report;
    use futures::{future, lock};
    use once_cell::sync::OnceCell;
    use std::env;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;
    use save_dweb_backend::backend::Backend;
    use save_dweb_backend::common::DHTEntity;
    use save_dweb_backend::constants as dweb;
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
    use crate::status_updater::{update_status, update_extended_status, SnowbirdServiceStatus};

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
            Backend::new(backend_path, 8080).expect("Generic reason")
        ))
    }

    // trait IntoBackendResult<T> {
    //     fn into_backend_result(self) -> Result<T, BackendError>;
    // }
    
    // impl<T, E: std::error::Error + Send + Sync + 'static> IntoBackendResult<T> for Result<T, E> {
    //     fn into_backend_result(self) -> Result<T, BackendError> {
    //         self.map_err(|e| BackendError::ThirdPartyError(Box::new(e)))
    //     }
    // }

    fn create_veilid_cryptokey_from_base64(key_string: &str) -> Result<CryptoKey, Box<dyn std::error::Error>> {
        // Decode base64url string to bytes
        let key_vec = base64_url::decode(key_string)?;
        
        let key_array: [u8; 32] = key_vec.try_into()
            .map_err(|_| "Key must be exactly 32 bytes long")?;
        
        // Create a CryptoKey from the bytes
        let crypto_key = CryptoKey::new(key_array);
        
        Ok(crypto_key)
    }

    #[get("/status")]
    async fn status() -> impl Responder {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "running",
            "version": *VERSION
        }))
    }

    #[get("/api/groups")]
    async fn get_groups() -> AppResult<impl Responder> {
        // update_status(SnowbirdServiceStatus::Processing);

        let mut backend = get_backend().await?;

        let groups = backend.list_groups().await.unwrap();

            // .map_err(|e| {
            //     eprintln!("Error listing groups: {:?}", e);
            //     actix_web::error::ErrorInternalServerError("Failed to retrieve groups")
            // });
    
        let group_data_futures: Vec<_> = groups.iter().map(|group| async move {
            let name_result: Result<String, Report> = group.get_name().await;
            let name = name_result.unwrap_or_else(|e| format!("Error: {}", e));
            log_debug!(TAG, "Name = {}", "bob");
            
            json!({
                "groupId": group.id(),
                "key": group.dht_record.key()
            })
        }).collect();
    
        let group_data: Vec<serde_json::Value> = future::join_all(group_data_futures).await;
    
        // update_status(SnowbirdServiceStatus::Idle);

        Ok(HttpResponse::Ok().json(json!({ "groups": group_data })))
    }

    #[get("/api/groups/{group_id}")]
    async fn get_group(path: web::Path<String>) -> AppResult<impl Responder> {
        let mut backend = get_backend().await?;

        let group_id = path.into_inner();

        let key_string = "nN7W0-JiuhIcCWhy4Sw0J7mfDWWE9OtnCfAbLmwLbq0";
        let key = create_veilid_cryptokey_from_base64(key_string).unwrap();

        let group = backend.get_group(TypedKey::new(CRYPTO_KIND_VLD0, key)).await.expect(dweb::GROUP_NOT_FOUND);

        Ok(HttpResponse::Ok().json(json!({
            "groupId": group.id(),
            "key": group.dht_record.key()
        })))
    }

    #[post("/api/groups")]
    async fn create_group() -> AppResult<impl Responder> {
        log_info!(TAG, "step 10");

        let mut backend = get_backend().await?;

        log_info!(TAG, "step 20");

        let group = backend.create_group().await?;

        log_info!(TAG, "step 30");

        Ok(HttpResponse::Ok().json(json!({
            "groupId": group.id(),
            "key": group.dht_record.key()
        })))
    }

    #[patch("/api/groups/{group_id}")]
    async fn update_group(path: web::Path<String>) -> AppResult<impl Responder> {
        let mut backend = get_backend().await?;

        let group_id = path.into_inner();

        // let group = backend.get_group(con).await?;

        // group.set_name("foo").await.expect(dweb::UNABLE_TO_SET_GROUP_NAME);

        Ok(HttpResponse::Ok().json(json!({
            "name": "My Group"
        })))
    }

    #[get("/api/repos")]
    async fn get_repos() -> AppResult<impl Responder> {
        let mut backend = get_backend().await?;

        Ok(HttpResponse::Ok().json(json!({
            "name": "My Repo"
        })))
    }

    #[post("/api/repos")]
    async fn create_repos() -> AppResult<impl Responder> {
        let mut backend = get_backend().await?;

        let repo = backend.create_repo().await.expect("Unable to create repo");
        let repo_key = repo.get_id();
        let repo_name = "Test Repo";

        Ok(HttpResponse::Ok().json(json!({
            "repoId": repo.id,
            "key": repo.dht_record.key()
        })))
    }

    pub async fn start(backend_base_directory: &str, server_socket_path: &str) -> anyhow::Result<()> {
        log_debug!(TAG, "start_server: Using socket path: {:?}", server_socket_path);

        match update_extended_status(SnowbirdServiceStatus::BackendRunning, Some("hi")) {
            Ok(_) => {
                log_debug!(TAG, "status updated");
            }
            Err(e) => {
                log_error!(TAG, "Update error: {:?}", e);
            }
        }

        if env::var("HOME").is_err() {
            env::set_var("HOME", backend_base_directory);
        }
        
        let backend_path = Path::new(backend_base_directory);

        BACKEND.get_or_init(|| init_backend(backend_path));
        
        {
            let mut backend = get_backend().await?;

            backend.start().await.context("Backend failed to start");
        }

        // update_status(SnowbirdServiceStatus::BackendRunning);

        let web_server = HttpServer::new(move || {
            // update_status(SnowbirdServiceStatus::WebServerInitializing);

            App::new()
            .service(status)
            .service(get_group)
            .service(get_groups)
            .service(create_group)
            .service(update_group)
            .service(get_repos)
        })
        .bind_uds(server_socket_path)?
        .run();

        // update_status(SnowbirdServiceStatus::WebServerRunning);

        // This one doesn't return, so we notify success before we get here.
        //
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