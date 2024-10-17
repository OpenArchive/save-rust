use anyhow::Result;

#[cfg(target_os = "macos")]
use crate::mac;

#[cfg(target_os = "macos")]
#[actix_web::main]
async fn main() -> Result<()> {
    mac::run().await
}

#[cfg(not(target_os = "macos"))]
fn main() {
    // This function will never be called on Android,
    // but it's needed to satisfy the Rust compiler
    unimplemented!("This binary is not meant to be run on Android")
}
