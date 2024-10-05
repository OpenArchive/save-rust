#![allow(clippy::result_large_err)] // Allows for larger error types in Result

use std::result::Result as StdResult;
use std::sync::{Arc, Mutex, Once};
use jni::AttachGuard;
use jni::JavaVM;
use jni::objects::{GlobalRef, JClass};
use jni::JNIEnv;
use once_cell::sync::Lazy;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JniError {
    #[error("JNI initialization error: {0}")]
    InitializationError(String),

    #[error("JNI error: {0}")]
    JniError(#[from] jni::errors::Error),

    #[error("Thread attachment error: {0}")]
    ThreadAttachError(String),

    #[allow(dead_code)]
    #[error("String conversion error: {0}")]
    StringConversionError(String),
}

pub type JniResult<T> = StdResult<T, JniError>;

static JAVA_VM: Lazy<Arc<Mutex<Option<JavaVM>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));
static CLASS: Lazy<Arc<Mutex<Option<GlobalRef>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));
static INIT: Once = Once::new();

#[allow(dead_code)]
pub fn get_java_vm() -> JniResult<JavaVM> {
    let jvm_locked = JAVA_VM.lock();
    let jvm = jvm_locked.as_ref().unwrap();
    let env = jvm.as_ref().unwrap().attach_current_thread_as_daemon().unwrap();
    let vm = env.get_java_vm();
    return Ok(vm?);
}

pub fn init_jni(env: &JNIEnv, class: JClass) -> JniResult<()> {
    INIT.call_once(|| {
        if let Err(e) = init_jni_inner(env, class) {
            eprintln!("Failed to initialize JNI: {e}");
        }
    });
    Ok(())
}

fn init_jni_inner(env: &JNIEnv, class: JClass) -> JniResult<()> {
    let java_vm = env.get_java_vm()?;
    let global_class = env.new_global_ref(class)?;
    
    *JAVA_VM.lock()
        .map_err(|e| JniError::InitializationError(format!("Failed to acquire JavaVM lock: {e}")))?
        = Some(java_vm);
    
    *CLASS.lock()
        .map_err(|e| JniError::InitializationError(format!("Failed to acquire class lock: {e}")))?
        = Some(global_class);
    
    Ok(())
}

#[allow(dead_code)]
pub fn with_java_vm<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&JavaVM) -> R,
{
    JAVA_VM.lock().ok()?.as_ref().map(f)
}

#[allow(dead_code)]
pub fn with_class<F, R>(f: F) -> JniResult<R>
where
    F: FnOnce(&GlobalRef) -> JniResult<R>,
{
    let class_guard = CLASS.lock()
        .map_err(|_| JniError::InitializationError("Failed to acquire class lock".into()))?;
    
    let class = class_guard.as_ref()
        .ok_or_else(|| JniError::InitializationError("Class not initialized".into()))?;

    f(class)
}

// fn with_env<F, R>(env: &mut JNIEnv, f: F) -> Result<R, Box<dyn Error>>
// where
//     F: FnOnce(AttachGuard) -> JniResult<R>,
// {
//     let env_ptr = env.get_native_interface();
//     let new_env = unsafe { JNIEnv::from_raw(env_ptr).unwrap() };
//     f(new_env)
// }

pub fn with_env<F, R>(f: F) -> JniResult<R>
where
    F: FnOnce(AttachGuard) -> JniResult<R>,
{
    let vm_guard = JAVA_VM.lock()
        .map_err(|e| JniError::ThreadAttachError(format!("Failed to acquire JavaVM lock: {e}")))?;
    
    let vm = vm_guard.as_ref()
        .ok_or_else(|| JniError::InitializationError("JavaVM not initialized".into()))?;

    let env = vm.attach_current_thread()
        .map_err(|e| JniError::ThreadAttachError(format!("Failed to attach thread: {e}")))?;

    f(env)
}

// pub fn print_class_name() -> Result<()> {
//     with_env(|env| {
//         let class = CLASS.lock()
//             .map_err(|_| JniError::InitializationError("Failed to acquire class lock".into()))?
//             .as_ref()
//             .ok_or(JniError::InitializationError("Class not initialized".into()))?;

//         // Get the Class object from our GlobalRef
//         let class_object = env.call_method(class, "getClass", "()Ljava/lang/Class;", &[])?
//             .l()?;

//         // Call getName() on the Class object
//         let name: JString = env.call_method(class_object, "getName", "()Ljava/lang/String;", &[])?
//             .l()?.into();

//         // Convert the Java string to a Rust string immediately
//         let class_name = env.get_string(&name)
//             .map_err(|e| JniError::StringConversionError(format!("Failed to convert class name: {e}")))?
//             .to_string_lossy()
//             .into_owned();

//         println!("Class name: {class_name}");
//         Ok(())
//     })
// }