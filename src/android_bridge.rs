#![allow(unused)]
use crate::constants::TAG;
use crate::jni_globals;
use crate::logging::android_log;
use crate::server;
use crate::server::start;
use crate::{log_debug, log_error, log_info};
use jni::errors::Result as JniResult;
use jni::sys::{jint, jstring};
use jni::{
    objects::GlobalRef, objects::JClass, objects::JMethodID, objects::JObject, objects::JString,
    objects::JValue, objects::JValueGen, JNIEnv, JavaVM,
};
use lazy_static::lazy_static;
use std::error::Error;
use std::sync::{Arc, Mutex, Once};
use std::thread;
use veilid_core::veilid_core_setup_android;
use std::time::Duration;


trait IntoJObject {
    fn into_jobject(&self) -> JObject;
}

impl IntoJObject for GlobalRef {
    fn into_jobject(&self) -> JObject {
        unsafe { JObject::from_raw(self.as_raw()) }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_initializeRustService(
    env: JNIEnv,
    class: JClass,
) {
    // match jni_globals::setup_android(env, class) {
    //     Ok(_) => log_debug!(TAG, "Rust service initialized successfully"),
    //     Err(e) => log_error!(TAG, "Failed to initialize Rust service: {:?}", e),
    // }

    log_info!(TAG, "SnowbirdBridge initialized");
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_startServer(
    mut env: JNIEnv,
    clazz: JClass,
    context: JObject,
    backend_base_directory: JString,
    server_socket_path: JString,
) -> jstring {
    let env_ptr = env.get_native_interface();

    log_debug!(TAG, "Bridge: starting");

    match setup_jni_environments(&mut env, context, clazz) {
        Ok(_) => {
            log_debug!(TAG, "JNI stuff successful");
        }
        Err(e) => {
            log_error!(TAG, "Error doing JNI stuff: {:?}", e);
        }
    }

    let backend_base_directory: String = env
        .get_string(&backend_base_directory)
        .expect("Couldn't get socket path string")
        .into();

    let server_socket_path: String = env
        .get_string(&server_socket_path)
        .expect("Couldn't get socket path string")
        .into();

    let backend_base_directory_clone = backend_base_directory.clone();
    let server_socket_path_clone = server_socket_path.clone();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            start(&backend_base_directory_clone, &server_socket_path_clone)
                .await
                .unwrap();
        });
    });

    log_debug!(TAG, "Bridge startup complete.");

    let output = env
        .new_string(format!(
            "Server started on Unix socket: {}",
            server_socket_path
        ))
        .expect("Couldn't create java string!");

    output.into_raw()
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_stopServer(
    mut env: JNIEnv,
    _clazz: JClass,
    ctx: JObject,
) -> jstring {
    log_debug!(TAG, "Bridge: stopping server");

    // Create a runtime to handle async operations
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    // Stop the backend server and clean up Veilid API
    let stop_result = runtime.block_on(async {
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
                
                Ok(())
            }
            Err(e) => {
                log_error!(TAG, "Error stopping server: {:?}", e);
                Err(e)
            }
        }
    });

    // Create response string based on result
    let response = match stop_result {
        Ok(_) => "Server stopped successfully",
        Err(_) => "Error stopping server",
    };

    let output = env
        .new_string(response)
        .expect("Couldn't create java string!");

    output.into_raw()
}

fn with_env<F, R>(env: &mut JNIEnv, f: F) -> Result<R, Box<dyn Error>>
where
    F: FnOnce(JNIEnv) -> Result<R, Box<dyn Error>>,
{
    let env_ptr = env.get_native_interface();
    let new_env = unsafe { JNIEnv::from_raw(env_ptr).unwrap() };
    f(new_env)
}

fn setup_jni_environments(
    env: &mut JNIEnv,
    context: JObject,
    clazz: JClass,
) -> Result<(), Box<dyn Error>> {
    with_env(env, |env| Ok(jni_globals::init_jni(&env, clazz)));

    let global_context = env.new_global_ref(context)?;

    // Use a new JNIEnv for jni_smoke_test
    with_env(env, |env| {
        jni_smoke_test(env, global_context.into_jobject())
    })?;

    // Use another new JNIEnv for veilid_core_setup_android
    with_env(env, |env| {
        veilid_core_setup_android(env, global_context.into_jobject());
        Ok(())
    })?;

    Ok(())
}

fn jni_smoke_test<'local>(
    mut env: JNIEnv<'local>,
    context: JObject<'local>,
) -> Result<(), Box<dyn std::error::Error>> {
    let class_name = "net/opendasharchive/openarchive/services/snowbird/SnowbirdBridge";
    let method_name = "updateStatusFromRust";
    let method_signature = "(ILjava/lang/String;)V";

    // Example status code
    let status_code = 1;

    // Create a JValue for the String parameter (can be null)
    let error_message = env.new_string("Test error message")?;
    let error_message_jvalue = JValue::Object(&error_message);

    // Call the static method
    env.call_static_method(
        class_name,
        method_name,
        method_signature,
        &[JValue::Int(status_code), error_message_jvalue],
    )?;

    Ok(())
}
