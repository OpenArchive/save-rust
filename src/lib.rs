use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_web::get;
use actix_web::post;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
struct Message {
    content: String,
}

mod server {
    use super::*;

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

    pub async fn start_server(socket_path: &str) -> std::io::Result<()> {
        HttpServer::new(|| {
            App::new()
            .service(hello)
            .service(echo)
            .service(status)
        })
        .bind_uds(&socket_path).unwrap()
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
    pub extern "system" fn Java_net_opendasharchive_openarchive_features_main_RustyServerManager_startServer(env: JNIEnv, _: JClass, socket_path: JString) -> jstring {
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