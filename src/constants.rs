use once_cell::sync::Lazy;

pub const TAG: &str = "SnowbirdBridge";

pub static VERSION: Lazy<String> = Lazy::new(|| {
    env!("CARGO_PKG_VERSION").to_string()
});