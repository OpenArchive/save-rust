use jni::{objects::JObject, objects::JValue};
use jni::sys::jint;
use crate::log_debug;
use crate::logging::android_log;
use crate::constants::TAG;
use crate::jni_globals::{with_env, JniResult};

#[repr(i32)]
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum SnowbirdServiceStatus {
    BackendInitializing = 0,
    BackendRunning = 1,
    WebServerInitializing = 2,
    WebServerRunning = 3,
    Processing = 4,
    Idle = 5,
    Error = 6,
}

pub fn update_status(status: SnowbirdServiceStatus) -> JniResult<()> {
    log_debug!(TAG, "Updating status: {:?}", status);
    update_extended_status(status, Some("hi"))?;
    log_debug!(TAG, "Status update complete");
    Ok(())
}

pub fn update_extended_status(status: SnowbirdServiceStatus, error_message: Option<&str>) -> JniResult<()> {
    let class_name = "net/opendasharchive/openarchive/services/snowbird/SnowbirdBridge";
    let method_name = "updateStatusFromRust";
    let method_signature = "(ILjava/lang/String;)V";
    let status_code: jint = status as jint;

    // Assume we have a function to get the JavaVM
    // let vm = get_java_vm()?;

    // Attach the current thread to get a JNIEnv
    // let env = vm.attach_current_thread()?;

    with_env(|mut env| {
        log_debug!(TAG, "Got env");

        // Create the error string
        let error_jstring = error_message
            .map(|msg| env.new_string(msg))
            .transpose()?;

        log_debug!(TAG, "Got error string");

        let null_obj = JObject::null();
        let error_jvalue = error_jstring
            .as_ref()
            .map_or(JValue::Object(&null_obj), |s| JValue::Object(s.as_ref()));
    
        // Find the class
        let class = env.find_class(class_name)?;

        log_debug!(TAG, "Got class");

        env.call_static_method(
            class,
            method_name,
            method_signature,
            &[JValue::Int(status_code), error_jvalue]
        )?;

        Ok(())
    })?;

    Ok(())
}

