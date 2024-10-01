use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use crate::server::server::start_server;

#[no_mangle]
pub extern "system" fn Java_net_opendasharchive_openarchive_services_snowbird_SnowbirdBridge_startServer(env: JNIEnv, _: JClass, socket_path: JString) -> jstring {
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
            start_server(&thread_socket_path).await.unwrap();
        });
    });

    let output = env
        .new_string(format!("Server started on Unix socket: {}", socket_path))
        .expect("Couldn't create java string!");

    output.into_raw()
}
