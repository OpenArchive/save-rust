#[cfg(target_os = "android")]
pub mod android_bridge;

pub mod logging;

mod server {
    use actix_web::{web, App, HttpResponse, HttpServer, Responder, Error};
    use actix_web::error::ErrorInternalServerError;
    use actix_web::{get, post};
    use crate::log_debug;
    use eyre::Report;
    use futures::future;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;
    use save_dweb_backend::backend::Backend;
    use save_dweb_backend::common::DHTEntity;
    use serde_json::json;
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};

    #[link(name = "log")]
    extern "C" {
        pub fn __android_log_print(prio: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
    }

    pub fn android_log(prio: i32, tag: &str, msg: &str) {
        let tag = CString::new(tag).unwrap();
        let msg = CString::new(msg).unwrap();
        unsafe {
            __android_log_print(prio, tag.as_ptr(), msg.as_ptr());
        }
    }

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
    
        let group_data: Vec<serde_json::Value> = future::join_all(group_data_futures).await;
    
        Ok(HttpResponse::Ok().json(json!({ "groups": group_data })))
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
        
        let backend_path = Path::new("/data/user/0/net.opendasharchive.openarchive.debug/files/backend");

        let backend = Arc::new(TokioMutex::new(
            Backend::new(backend_path, 8080).expect("Unable to create Backend")
        ));
        
        log_debug!("RustNative", "start_verver: step 10");

        // {
        //     let mut backend_guard = backend.lock().await;
        //     if let Err(e) = backend_guard.start().await {
        //         ErrorInternalServerError(format!("Failed to start server: {}", e));
        //         return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        //     }
        // } 

        log_debug!("RustNative", "start_verver: step 20");

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
