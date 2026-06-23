#![allow(clippy::result_large_err)] // Allows for larger error types in Result

use jni::objects::{Global, JClass};
use jni::vm::JavaVM;
use jni::Env;
use once_cell::sync::{Lazy, OnceCell};
use std::result::Result as StdResult;
use std::sync::{Arc, Mutex};
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
static CLASS: Lazy<Arc<Mutex<Option<Global<JClass<'static>>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));
static INIT: OnceCell<()> = OnceCell::new();

#[allow(dead_code)]
pub fn get_java_vm() -> JniResult<JavaVM> {
    let jvm_locked = JAVA_VM.lock().map_err(|e| {
        JniError::InitializationError(format!("Failed to acquire JavaVM lock: {e}"))
    })?;
    jvm_locked
        .as_ref()
        .cloned()
        .ok_or_else(|| JniError::InitializationError("JavaVM not initialized".into()))
}

pub fn init_jni(env: &mut Env, class: JClass) -> JniResult<()> {
    INIT.get_or_try_init(|| init_jni_inner(env, class))
        .map(|_| ())
}

fn init_jni_inner(env: &mut Env, class: JClass) -> JniResult<()> {
    let java_vm = env.get_java_vm()?;
    let global_class = env.new_global_ref(class)?;

    *JAVA_VM.lock().map_err(|e| {
        JniError::InitializationError(format!("Failed to acquire JavaVM lock: {e}"))
    })? = Some(java_vm);

    *CLASS.lock().map_err(|e| {
        JniError::InitializationError(format!("Failed to acquire class lock: {e}"))
    })? = Some(global_class);

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
    F: FnOnce(&Global<JClass<'static>>) -> JniResult<R>,
{
    let class_guard = CLASS
        .lock()
        .map_err(|_| JniError::InitializationError("Failed to acquire class lock".into()))?;

    let class = class_guard
        .as_ref()
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
    F: FnOnce(&mut Env) -> JniResult<R>,
{
    let vm_guard = JAVA_VM
        .lock()
        .map_err(|e| JniError::ThreadAttachError(format!("Failed to acquire JavaVM lock: {e}")))?;

    let vm = vm_guard
        .as_ref()
        .ok_or_else(|| JniError::InitializationError("JavaVM not initialized".into()))?;

    vm.attach_current_thread(f)
        .map_err(|e| JniError::ThreadAttachError(format!("Failed to attach thread: {e}")))
}
