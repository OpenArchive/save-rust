#![recursion_limit = "256"]

#[cfg(target_os = "android")]
pub mod android_bridge;

pub mod constants;
pub mod error;
pub mod jni_globals;
pub mod logging;
pub mod groups;
pub mod repos;
pub mod media;
pub mod server;
pub mod models;
pub mod status_updater;
pub mod utils;