#![allow(unused)]
use j4rs::{Instance, InvocationArg, Jvm, JvmBuilder};
use jni::{objects::GlobalRef, objects::JClass, objects::JObject, objects::JMethodID, objects::JString, objects::JValueGen, JNIEnv, JavaVM};
use jni::sys::{jint, jstring};
use lazy_static::lazy_static;
use log::{debug, error, info};
use std::sync::{Arc, Once, Mutex};
use veilid_core::veilid_core_setup_android;
use crate::server::server::start;
use crate::{log_debug, log_info, log_error};
use crate::logging::android_log;
use crate::constants::TAG;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
enum SnowbirdServiceStatus {
    Initializing = 0,
    Running = 1,
    Processing = 2,
    Paused = 3,
    Error = 4,
    Completed = 5,
}

static INIT: Once = Once::new();
static JAVA_VM: Mutex<Option<JavaVM>> = Mutex::new(None);
static CLASS: Mutex<Option<GlobalRef>> = Mutex::new(None);
static METHOD_ID: Mutex<Option<JMethodID>> = Mutex::new(None);

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_initializeRustService(
    env: JNIEnv,
    class: JClass,
) {
    INIT.call_once(|| {
        let java_vm = env.get_java_vm().expect("Failed to get JavaVM");
        let global_class = env.new_global_ref(class).expect("Failed to create global reference");
        
        *JAVA_VM.lock().unwrap() = Some(java_vm);
        *CLASS.lock().unwrap() = Some(global_class);
    });

    // send_status_update(env, SnowbirdServiceStatus::Initializing, None);
    info!(target: &TAG, "SnowbirdBridge initialized");
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_startServer(
    mut env: JNIEnv,
    _clazz: JClass,
    ctx: JObject,
    backend_base_directory: JString,
    server_socket_path: JString
) -> jstring {
    let env_ptr = env.get_native_interface();
    let new_env = unsafe { JNIEnv::from_raw(env_ptr).unwrap() };

    log_debug!("RustNative", "bridge: starting");

    // send_status_update(SnowbirdServiceStatus::Initializing, None);

    veilid_core_setup_android(new_env, ctx);

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
            start(&backend_base_directory_clone, &server_socket_path_clone).await.unwrap();
        });
    });

    let output = env
        .new_string(format!("Server started on Unix socket: {}", server_socket_path))
        .expect("Couldn't create java string!");

    output.into_raw()
}

fn send_status_update(status: SnowbirdServiceStatus, error_message: Option<&str>) {
    log_debug!("RustNative", "bridge: send_status_update");

    let jvm = JvmBuilder::new().build().expect("foo");

    log_debug!("RustNative", "bridge: send_status_update: 10");

    jvm.invoke_static(
        "net.opendasharchive.openarchive.services.snowbird.SnowbirdBridge",
        "onStatusUpdate",
        &[
            InvocationArg::try_from(status as i32).expect("foo"),
            InvocationArg::try_from("bob").expect("foo")
        ]
    );

    log_debug!("RustNative", "bridge: send_status_update: 20");

    match status {
        SnowbirdServiceStatus::Error => error!(target: &TAG, "Status update: Error - {}", error_message.unwrap_or("Unknown error")),
        _ => info!(target: &TAG, "Status update: {:?}", status)
    }
}