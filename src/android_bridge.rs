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

// use jni::JNIEnv;
// use jni::objects::{JClass, JString};
// use jni::sys::jstring;
// use std::fs;
// use std::path::Path;
// use tokio::runtime::Runtime;
// use tokio::task;
// use std::time::Duration;
// use crate::log_debug;
// use crate::server::start_server;

// async fn start_server_task(socket_path: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//     let path = Path::new(&socket_path);
    
//     log_debug!("RustNative", "Using socket path: {:?}", path);

//     if path.exists() {
//         log_debug!("RustNative", "Path exists. Removing");
//         let path_clone = path.to_path_buf();
//         task::spawn_blocking(move || fs::remove_file(path_clone))
//             .await??;
//     }

//     // Ensure the parent directory exists
//     // if let Some(parent) = path.parent() {
//     //     log_debug!("RustNative", "Parent exists");
//     //     fs::create_dir_all(parent)?;
//     // }

//     start_server(&socket_path).await?;

//     Ok(())
// }

// #[no_mangle]
// pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_startServer(
//     mut env: JNIEnv,
//     _: JClass,
//     socket_path: JString
// ) -> jstring {
//     log_debug!("RustNative", "Starting server from Android bridge");

//     let socket_path: String = match env.get_string(&socket_path) {
//         Ok(s) => s.into(),
//         Err(e) => {
//             log_debug!("RustNative", "Failed to get socket path: {:?}", e);
//             return env.new_string("Failed to get socket path").unwrap().into_raw();
//         }
//     };

//     log_debug!("RustNative", "Received socket path: {}", socket_path);

//     // Create a new Tokio runtime
//     let runtime = match Runtime::new() {
//         Ok(rt) => rt,
//         Err(e) => {
//             log_debug!("RustNative", "Failed to create Tokio runtime: {:?}", e);
//             return env.new_string("Failed to create Tokio runtime").unwrap().into_raw();
//         }
//     };

//     // Start the server task
//     let server_future = start_server_task(socket_path.clone());

//     // Wait for a short time to catch immediate failures
//     let result = runtime.block_on(async {
//         tokio::time::timeout(Duration::from_millis(100), server_future).await
//     });

//     let output = match result {
//         Ok(Ok(_)) => {
//             log_debug!("RustNative", "Server started successfully");
//             format!("Server started on Unix socket: {}", socket_path)
//         }
//         Ok(Err(e)) => {
//             log_debug!("RustNative", "Server failed to start: {:?}", e);
//             format!("Failed to start server: {:?}", e)
//         }
//         Err(_) => {
//             log_debug!("RustNative", "Server start pending");
//             "Server start initiated, but status unknown".to_string()
//         }
//     };

//     match env.new_string(&output) {
//         Ok(s) => s.into_raw(),
//         Err(e) => {
//             log_debug!("RustNative", "Failed to create output string: {:?}", e);
//             env.new_string("Internal error").unwrap().into_raw()
//         }
//     }
// }