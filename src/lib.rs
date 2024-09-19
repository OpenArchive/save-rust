use actix_web::{web, App, HttpResponse, HttpServer, Responder, Error};
use actix_web::get;
use actix_web::post;
use eyre::Report;
use futures::future;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use save_dweb_backend::backend::Backend;
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize)]
struct Message {
    content: String,
}

mod server {
    use save_dweb_backend::common::DHTEntity;

    use super::*;

    struct AppState {
        backend: Arc<Mutex<Backend>>,
    }

    #[get("/")]
    async fn hello() -> impl Responder {
        HttpResponse::Ok().body("Hello from Rust server!")
    }

    #[post("/echo")]
    async fn echo(message: web::Json<Value>) -> impl Responder {
        HttpResponse::Ok().json(message.0)
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
        let backend = data.backend.lock().map_err(|_| {
            actix_web::error::ErrorInternalServerError("Failed to acquire backend lock")
        })?;

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
        let mut backend = data.backend.lock().map_err(|_| {
            actix_web::error::ErrorInternalServerError("Failed to acquire backend lock")
        })?;
    
        let group = backend.create_group().await
            .map_err(|e| {
                eprintln!("Error creating group: {:?}", e);
                actix_web::error::ErrorInternalServerError(format!("Failed to create group: {}", e))
            })?;
    
        Ok(HttpResponse::Ok().json(json!({
            "name": "My Group"
        })))
    }

    pub async fn start_server(socket_path: &str) -> std::io::Result<()> {
        let backend = Arc::new(Mutex::new(
            Backend::new(socket_path.as_ref(), 8080).expect("Unable to create Backend")
        ));
        
        HttpServer::new(move || {
            let app_state = web::Data::new(AppState {
                backend: backend.clone(),
            });

            App::new()
            .app_data(app_state)
            .service(hello)
            .service(echo)
            .service(status)
            .service(get_groups)
            .service(create_group)
        })
        .bind_uds(socket_path)?
        .run()
        .await
    }
}

#[cfg(target_os = "android")]
mod android {
    use super::*;
    use jni::JNIEnv;
    use jni::objects::{JClass, JString};
    use jni::sys::jstring;
    use std::fs;

    #[no_mangle]
    pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_startServer(env: JNIEnv, _: JClass, socket_path: JString) -> jstring {
        let socket_path: String = unsafe {
            env
                .get_string_unchecked(&socket_path)
                .expect("Couldn't get socket path string")
                .into()
        };

        let thread_socket_path = socket_path.clone();

        if fs::metadata(&socket_path).is_ok() {
            fs::remove_file(&thread_socket_path).unwrap();
        }

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                server::start_server(&thread_socket_path).await.unwrap();
            });
        });
    
        let output = env
            .new_string(format!("Server started on Unix socket: {}", socket_path))
            .expect("Couldn't create java string!");

        output.into_raw()
    }
}

#[cfg(feature = "ios")]
mod ios { 
    // Placeholder
 }