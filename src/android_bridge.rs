#![allow(unused)]
use crate::constants::TAG;
use crate::jni_globals;
use crate::logging::android_log;
use crate::server;
use crate::server::start;
use crate::{log_debug, log_error, log_info};
use jni::errors::Result as JniResult;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::jni_sig;
use jni::jni_str;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jint, jstring};
use jni::{Env, EnvUnowned};
use std::time::Duration;
use veilid_core::veilid_core_setup_android;

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_initializeRustService(
    mut env: EnvUnowned,
    _class: JClass,
) {
    env.with_env(|_env| -> JniResult<()> {
        // match jni_globals::setup_android(env, class) {
        //     Ok(_) => log_debug!(TAG, "Rust service initialized successfully"),
        //     Err(e) => log_error!(TAG, "Failed to initialize Rust service: {:?}", e),
        // }

        log_info!(TAG, "SnowbirdBridge initialized");
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>();
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_startServer(
    mut env: EnvUnowned,
    clazz: JClass,
    context: JObject,
    backend_base_directory: JString,
    server_socket_path: JString,
) -> jstring {
    log_debug!(TAG, "Bridge: starting");

    // Initialize JNI globals, smoke-test the Java callback, and read Java args while
    // EnvUnowned is still available. veilid_core_setup_android consumes env/context.
    let (backend_base_directory, server_socket_path, output) = env
        .with_env(|env| -> JniResult<(String, String, jstring)> {
            jni_globals::init_jni(env, clazz).map_err(|e| {
                jni::errors::Error::ParseFailed(format!("Failed to initialize JNI globals: {e}"))
            })?;
            jni_smoke_test(env)?;

            let backend_base_directory = backend_base_directory.try_to_string(env)?;
            let server_socket_path = server_socket_path.try_to_string(env)?;
            let output = JString::from_str(
                env,
                format!("Server started on Unix socket: {server_socket_path}"),
            )?
            .into_raw();

            log_debug!(TAG, "JNI stuff successful");

            Ok((backend_base_directory, server_socket_path, output))
        })
        .resolve::<ThrowRuntimeExAndDefault>();

    // resolve() throws to Java and returns null on failure; do not start Veilid or the server.
    if output.is_null() {
        return output;
    }

    veilid_core_setup_android(env, context);

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            start(&backend_base_directory, &server_socket_path)
                .await
                .unwrap();
        });
    });

    log_debug!(TAG, "Bridge startup complete.");

    output
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_stopServer(
    mut env: EnvUnowned,
    _clazz: JClass,
    _ctx: JObject,
) -> jstring {
    log_debug!(TAG, "Bridge: stopping server");

    let stop_ok = env
        .with_env(|_env| -> JniResult<bool> {
            // Create a runtime to handle async operations
            let runtime = tokio::runtime::Runtime::new().unwrap();

            // Stop the backend server and clean up Veilid API
            runtime.block_on(async {
                // First stop the backend
                match server::stop().await {
                    Ok(_) => {
                        log_info!(TAG, "Backend stopped successfully");

                        // Get the backend to access Veilid API
                        if let Ok(backend) = server::get_backend().await {
                            // Shutdown Veilid API
                            if let Some(veilid_api) = backend.get_veilid_api().await {
                                veilid_api.shutdown().await;
                                log_info!(TAG, "Veilid API shut down successfully");
                            }
                        }

                        // Add a small delay to ensure tasks complete
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        Ok(true)
                    }
                    Err(e) => {
                        log_error!(TAG, "Error stopping server: {:?}", e);
                        Ok(false)
                    }
                }
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>();

    // Create response string based on result
    let response = if stop_ok {
        "Server stopped successfully"
    } else {
        "Error stopping server"
    };

    env.with_env(|env| -> JniResult<jstring> {
        let output = JString::from_str(env, response)?;
        Ok(output.into_raw())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

fn jni_smoke_test(env: &mut Env) -> JniResult<()> {
    let class_name = "net/opendasharchive/openarchive/services/snowbird/SnowbirdBridge";
    let method_name = "updateStatusFromRust";
    let method_signature = "(ILjava/lang/String;)V";

    // Example status code
    let status_code = 1;

    // Create a JValue for the String parameter (can be null)
    let error_message = env.new_string("Test error message")?;

    // Call the static method
    env.call_static_method(
        jni_str!("net/opendasharchive/openarchive/services/snowbird/SnowbirdBridge"),
        jni_str!("updateStatusFromRust"),
        jni_sig!("(ILjava/lang/String;)V"),
        &[JValue::Int(status_code), JValue::Object(&error_message)],
    )?;

    Ok(())
}