#[cfg(target_os = "android")]
mod android_bridge;

mod constants;
mod error;
mod jni_globals;
mod logging;
mod server;
mod status_updater;