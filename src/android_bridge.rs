use jni::{objects::JClass, objects::JObject, objects::JString, JNIEnv};
use jni::sys::jstring;
use ndk_context::android_context;
use veilid_core::veilid_core_setup_android;
use crate::server::server::start;

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_startServer(env: JNIEnv, _clazz: JClass, ctx: JObject, socket_path: JString) -> jstring {
    let env_ptr = env.get_native_interface();
    let new_env = unsafe { JNIEnv::from_raw(env_ptr).unwrap() };

    veilid_core_setup_android(new_env, ctx);

    let socket_path: String = unsafe {
        env
            .get_string_unchecked(&socket_path)
            .expect("Couldn't get socket path string")
            .into()
    };

    let thread_socket_path = socket_path.clone();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            start(&thread_socket_path).await.unwrap();
        });
    });

    let output = env
        .new_string(format!("Server started on Unix socket: {}", socket_path))
        .expect("Couldn't create java string!");

    output.into_raw()
}
