#![recursion_limit = "256"]

#[cfg(target_os = "android")]
pub mod android_bridge;

#[cfg(target_os = "android")]
pub mod jni_globals;

#[cfg(target_os = "macos")]
pub mod mac;

pub mod actix_route_dumper;
pub mod constants;
pub mod error;
pub mod logging;
pub mod groups;
pub mod repos;
pub mod media;
pub mod server;
pub mod models;
pub mod utils;

// Common function for both Android and non-Android
// pub async fn run_server(host: &str, port: &str) -> Result<()> {
//     start(host, port).await
// }
