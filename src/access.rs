//! Access to open sqlite3 database by filename.
//!
//! The `core` module requires explicit authority to access files and such,
//! following the principle of least authority.
//!
//! This module provides the privileged functions to create such authorities.
//!
//! TODO: move `mod access` to its own crate so that linking to `sqlite3` doesn't
//! bring in this ambient authority..

use libc::c_int;

use ffi;
use core::Access;

pub fn filename_access(filename: String) -> Access {
    proc(db: *mut *mut ffi::sqlite3) -> c_int {
        filename.with_c_str({
            |filename| unsafe { ffi::sqlite3_open(filename, db) }
        })
    }
}


#[cfg(test)]
mod tests {
    use super::filename_access;
    use core::DatabaseConnection;

    #[test]
    fn open_file_db() {
        DatabaseConnection::connect(filename_access("/tmp/db1".to_string())).unwrap();
    }
}

// Local Variables:
// flycheck-rust-crate-root: "lib.rs"
// End:
