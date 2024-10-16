use std::ffi::CString;
use std::os::raw::{c_char, c_int};

#[link(name = "log")]
extern "C" {
    pub fn __android_log_print(prio: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
}

#[cfg(target_os = "android")]
pub fn android_log(prio: i32, tag: &str, msg: &str) {
    let tag = CString::new(tag).unwrap();
    let msg = CString::new(msg).unwrap();
    unsafe {
        __android_log_print(prio, tag.as_ptr(), msg.as_ptr());
    }
}

#[cfg(not(target_os = "android"))]
pub fn android_log(level: i32, tag: &str, msg: &str) {
    println!("[{:?}] {}: {}", level, tag, msg);
}

// Define log levels
#[allow(dead_code)] pub const LOG_LEVEL_DEBUG: i32 = 3;
#[allow(dead_code)] pub const LOG_LEVEL_INFO: i32 = 4;
#[allow(dead_code)] pub const LOG_LEVEL_WARN: i32 = 5;
#[allow(dead_code)] pub const LOG_LEVEL_ERROR: i32 = 6;

// Main logging macro
#[macro_export]
macro_rules! android_log_print {
    ($level:expr, $tag:expr, $($arg:tt)*) => {
        android_log($level, $tag, &format!("[{}:{}] {}", file!(), line!(), format_args!($($arg)*)))
    }
}

// Convenience macros for different log levels
#[macro_export]
macro_rules! log_debug {
    ($tag:expr, $($arg:tt)*) => { $crate::android_log_print!($crate::logging::LOG_LEVEL_DEBUG, $tag, $($arg)*) }
}

#[macro_export]
macro_rules! log_info {
    ($tag:expr, $($arg:tt)*) => { $crate::android_log_print!($crate::logging::LOG_LEVEL_INFO, $tag, $($arg)*) }
}

#[macro_export]
macro_rules! log_warn {
    ($tag:expr, $($arg:tt)*) => { $crate::android_log_print!($crate::logging::LOG_LEVEL_WARN, $tag, $($arg)*) }
}

#[macro_export]
macro_rules! log_error {
    ($tag:expr, $($arg:tt)*) => { $crate::android_log_print!($crate::logging::LOG_LEVEL_ERROR, $tag, $($arg)*) }
}