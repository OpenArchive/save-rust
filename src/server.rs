#![allow(unused)]
pub mod server {
    use actix_web::{web, App, HttpResponse, HttpServer, Responder, Error};
    use actix_web::{get, patch, post, put};
    use base64_url;
    use eyre::Report;
    use futures::future;
    use std::env;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;
    use save_dweb_backend::backend::Backend;
    use save_dweb_backend::common::DHTEntity;
    use save_dweb_backend::constants as dweb;
    use serde_json::json;
    use std::fs;
    use veilid_core::{
        vld0_generate_keypair, CryptoKey, TypedKey, VeilidUpdate, CRYPTO_KIND_VLD0, VALID_CRYPTO_KINDS
    };
    use crate::{log_debug, log_info, log_error};
    use crate::logging::android_log;
    use crate::constants::{self, TAG};

    struct AppState {
        backend: Arc<TokioMutex<Backend>>,
    }

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
            "version": env!("CARGO_PKG_VERSION")
        }))
    }

    #[get("/api/groups")]
    async fn get_groups(data: web::Data<AppState>) -> Result<impl Responder, Error> {
        let backend = data.backend.lock().await;

        let groups = backend.list_groups().await
            .map_err(|e| {
                eprintln!("Error listing groups: {:?}", e);
                actix_web::error::ErrorInternalServerError("Failed to retrieve groups")
            })?;
    
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
    
        Ok(HttpResponse::Ok().json(json!({ "groups": group_data })))
    }

    #[get("/api/groups/{group_id}")]
    async fn get_group(data: web::Data<AppState>, path: web::Path<String>) -> Result<impl Responder, Error> {
        let mut backend = data.backend.lock().await;
    
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
    async fn create_group(data: web::Data<AppState>) -> Result<impl Responder, Error> {
        let mut backend = data.backend.lock().await;
    
        let group = backend.create_group().await
            .map_err(|e| {
                eprintln!("Error creating group: {:?}", e);
                actix_web::error::ErrorInternalServerError(format!("Failed to create group: {}", e))
            })?;

            // Ok(HttpResponse::Ok().json(group));
    
        Ok(HttpResponse::Ok().json(json!({
            "groupId": group.id(),
            "key": group.dht_record.key()
        })))
    }

    #[patch("/api/groups/{group_id}")]
    async fn update_group(data: web::Data<AppState>, path: web::Path<String>) -> Result<impl Responder, Error> {
        let mut backend = data.backend.lock().await;
    
        let group_id = path.into_inner();

        // let group = backend.get_group(con).await?;

        // group.set_name("foo").await.expect(dweb::UNABLE_TO_SET_GROUP_NAME);

        Ok(HttpResponse::Ok().json(json!({
            "name": "My Group"
        })))
    }

    #[get("/api/repos")]
    async fn get_repos(data: web::Data<AppState>) -> Result<impl Responder, Error> {
        let mut backend = data.backend.lock().await;

        Ok(HttpResponse::Ok().json(json!({
            "name": "My Repo"
        })))
    }

    #[post("/api/repos")]
    async fn create_repos(data: web::Data<AppState>) -> Result<impl Responder, Error> {
        let mut backend = data.backend.lock().await;

        let repo = backend.create_repo().await.expect("Unable to create repo");
        let repo_key = repo.get_id();
        let repo_name = "Test Repo";

        Ok(HttpResponse::Ok().json(json!({
            "name": "My Repo"
        })))
    }

    pub async fn start(backend_base_directory: &str, server_socket_path: &str) -> std::io::Result<()> {
        log_debug!(TAG, "start_server: Using socket path: {:?}", server_socket_path);

        if env::var("HOME").is_err() {
            env::set_var("HOME", "/data/user/0/net.opendasharchive.openarchive.debug/files");
        }
        
        let backend_path = Path::new(backend_base_directory);

        let backend = Arc::new(TokioMutex::new(
            Backend::new(backend_path, 8080).expect("Unable to create Backend")
        ));
        
        let backend_clone = Arc::clone(&backend);

        match backend_clone.lock().await.start().await {
            Ok(_) => log_debug!(TAG, "Backend started successfully"),
            Err(e) => log_error!(TAG, "Failed to start backend: {}", e)
        }

        HttpServer::new(move || {
            let app_state = web::Data::new(AppState {
                backend: backend.clone(),
            });

            log_info!(TAG, "Server started");

            App::new()
            .app_data(app_state)
            .service(status)
            .service(get_group)
            .service(get_groups)
            .service(create_group)
            .service(update_group)
            .service(get_repos)
        })
        .bind_uds(server_socket_path)?
        .run()
        .await
    }
}