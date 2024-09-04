use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn rust_greeting(to: *const c_char) -> *mut c_char {
    let c_str = unsafe { CStr::from_ptr(to) };
    let recipient = match c_str.to_str() {
        Err(_) => "there",
        Ok(string) => string,
    };

    CString::new("Hello ".to_owned() + recipient).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn rust_greeting_free(s: *mut c_char) {
    unsafe {
        if s.is_null() { return }
        let _ = CString::from_raw(s);
    };
}

// Android-specific wrapper (only compiled when targeting Android)
#[cfg(target_os = "android")]
mod android {
    use super::*;
    use jni::JNIEnv;
    use jni::objects::{JClass, JString};
    use jni::sys::jstring;

    #[no_mangle]
    pub extern "system" fn Java_net_opendasharchive_openarchive_features_main_Rusty_rust_1greeting<'local>(
        mut env: JNIEnv<'local>,
        _: JClass<'local>,
        j_recipient: JString<'local>
    ) -> jstring {
        // Convert JString to Rust str
        let recipient = env.get_string(&j_recipient).expect("Invalid string input");
        let recipient_str = recipient.to_str().unwrap();

        // Call the core rust_greeting function
        let output = rust_greeting(recipient_str.as_ptr() as *const c_char);

        // Convert the result back to a jstring
        let output_jstring = env.new_string(unsafe { 
            CStr::from_ptr(output).to_str().unwrap() 
        }).expect("Couldn't create Java string!");

        // Free the CString created in rust_greeting
        rust_greeting_free(output);

        // Return the jstring
        output_jstring.into_raw()
    }
}
