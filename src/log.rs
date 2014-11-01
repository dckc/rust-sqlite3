//! Enable the SQLite error and warning log to help with debugging application problems.
//!

use libc::{c_char, c_int, c_void};
use std::ptr;
use std::option::Option;

use super::{SqliteResult};
use core::decode_result;

use ffi;

#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
enum SqliteConfig {
    SQLITE_CONFIG_LOG = 16,
}

pub type ErrorLogCallback =
    Option<extern "C" fn (p_arg: *mut c_void, err_code: c_int, z_msg: *const c_char)>;

/// Set up the error logging callback
///
/// cf [The Error And Warning Log](http://sqlite.org/errlog.html).
pub fn config_log(cb: ErrorLogCallback) -> SqliteResult<()> {
    let result = unsafe {
        let p_arg: *mut c_void = ptr::null_mut();
        ffi::sqlite3_config(SQLITE_CONFIG_LOG as i32, cb, p_arg)
    };
    decode_result(ptr::null_mut(), result, "sqlite3_config(SQLITE_CONFIG_LOG, ...)", None)
}

/// Write a message into the error log established by `config_log`.
pub fn log(err_code: c_int, msg: &str) {
    msg.with_c_str({
        |msg| unsafe { ffi::sqlite3_log(err_code, msg) }
    })
}

#[cfg(test)]
mod test_opening {
    use libc::{c_char, c_int, c_void};
    use ffi;
    use super::super::{SQLITE_NOTICE};

    extern "C" fn error_log_callback(_: *mut c_void, err_code: c_int, z_msg: *const c_char) {
        unsafe {
            println!("{}: {}", err_code, ::std::c_str::CString::new(z_msg, false));
        }
    }

    #[test]
    fn db_trace_callback() {
        unsafe { ffi::sqlite3_shutdown() };
        let result = super::config_log(Some(error_log_callback));
        match result {
            Ok(()) => (),
            Err(err) => panic!("error while configuring log: {}", err)
        }
        super::log(SQLITE_NOTICE as c_int, "message from rust-sqlite3")
    }
}