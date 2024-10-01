pub mod server {
    use actix_web::{web, App, HttpResponse, HttpServer, Responder, Error};
    use actix_web::{get, post};
    use eyre::Report;
    use futures::future;
    use std::env;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;
    use save_dweb_backend::backend::Backend;
    use save_dweb_backend::common::DHTEntity;
    use serde_json::json;
    use crate::log_debug;
    use crate::logging::android_log;

    struct AppState {
        backend: Arc<TokioMutex<Backend>>,
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
            
            json!({
                "name": name,
                "id": group.id(),
            })
        }).collect();
    
        let _group_data: Vec<serde_json::Value> = future::join_all(group_data_futures).await;
    
        Ok(HttpResponse::Ok().json(json!({ "groups": [{"name": "bob"}] })))
    }

    #[post("/api/groups")]
    async fn create_group(data: web::Data<AppState>) -> Result<impl Responder, Error> {
        let mut backend = data.backend.lock().await;
    
        let _group = backend.create_group().await
            .map_err(|e| {
                eprintln!("Error creating group: {:?}", e);
                actix_web::error::ErrorInternalServerError(format!("Failed to create group: {}", e))
            })?;
    
        Ok(HttpResponse::Ok().json(json!({
            "name": "My Group"
        })))
    }

    pub async fn start_server(socket_path: &str) -> std::io::Result<()> {
        log_debug!("RustNative", "start_server: Using socket path: {:?}", socket_path);
        
        let backend_path = Path::new(socket_path);

        let backend = Arc::new(TokioMutex::new(
            Backend::new(backend_path, 8080).expect("Unable to create Backend")
        ));
        
        log_debug!("RustNative", "start_verver: step 10");

        HttpServer::new(move || {
            let app_state = web::Data::new(AppState {
                backend: backend.clone(),
            });

            log_debug!("RustNative", "start_verver: step 30");

            App::new()
            .app_data(app_state)
            .service(status)
            .service(get_groups)
            .service(create_group)
        })
        .bind_uds(socket_path)?
        .run()
        .await
    }
}