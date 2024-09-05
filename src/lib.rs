use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Message {
    content: String,
}

async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello from Rust server on Android using Unix socket!")
}

async fn echo(message: web::Json<Message>) -> impl Responder {
    HttpResponse::Ok().json(message.0)
}

#[cfg(target_os = "android")]
mod android {
    use jni::JNIEnv;
    use jni::objects::{JClass, JString};
    use jni::sys::jstring;
    use actix_web::{web, App, HttpServer};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use std::fs;

    #[no_mangle]
    pub extern "system" fn Java_net_opendasharchive_openarchive_features_main_RustBridge_startServer(env: JNIEnv, _: JClass, socket_path: JString) -> jstring {
        
        let socket_path: String = unsafe {
            env.get_string_unchecked(&socket_path).expect("Couldn't get socket path string").into()
        };

        let runtime = tokio::runtime::Runtime::new().unwrap();
        
        runtime.block_on(async {
            // Remove the socket file if it already exists
            if fs::metadata(&socket_path).is_ok() {
                fs::remove_file(&socket_path).unwrap();
            }
    
            let server = HttpServer::new(|| {
                App::new()
                    .route("/echo", web::post().to(echo))
            })
            .bind_uds(&socket_path).unwrap()
            .run();
    
            let server = Arc::new(Mutex::new(Some(server)));
            let server_clone = server.clone();
    
            tokio::spawn(async move {
                if let Some(server) = server_clone.lock().await.take() {
                    server.await.unwrap();
                }
            });
        });
    
        let output = env
            .new_string(format!("Server started on Unix socket: {}", socket_path))
            .expect("Couldn't create java string!");
        output.into_raw()
    }

    async fn echo(message: web::Json<serde_json::Value>) -> web::Json<serde_json::Value> {
        message
    }

    #[no_mangle]
    pub extern "system" fn Java_net_opendasharchive_openarchive_features_main_RustBridge_echo<'local>(env: JNIEnv, _: JClass, input: JString) -> jstring {
        let input: String = unsafe {
            env.get_string_unchecked(&input).expect("Couldn't get java string!").into()
        };
        let output = format!("Echo: {}", input);
        let output = env.new_string(output).expect("Couldn't create java string!");
        output.into_raw()
    }
}