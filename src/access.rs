//! Access to open sqlite3 database by filename.
//!
//! The `core` module requires explicit authority to access files and such,
//! following the principle of least authority.
//!
//! This module provides the privileged functions to create such authorities.
//!
//! *TODO: move `mod access` to its own crate so that linking to `sqlite3` doesn't
//! bring in this ambient authority.*
#![unstable]

use libc::c_int;

use super::SqliteError;
use core::{Access, DatabaseConnection};
use ffi;

/// Open a database by filename.
///
/// *TODO: test for "Note that sqlite3_open() can be used to either
/// open existing database files or to create and open new database
/// files."*
/// 
///
/// Refer to [Opening A New Database][open] regarding URI filenames.
///
/// [open]: http://www.sqlite.org/c3ref/open.html
#[stable]
pub fn open(filename: String) -> Result<DatabaseConnection, (SqliteError, String)> {
    DatabaseConnection::new(filename_access(filename))
}

/// Create authorization to open a file as a database.
///
/// *The resulting proc() allocates an `sqlite3` structure
/// that is intended to be passed to `DatabaseConnection::new`.
/// Failure to do would result in a memory leak.*
///
/// *TODO: mark this unsafe?*
#[unstable]
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
        DatabaseConnection::new(filename_access("/tmp/db1".to_string())).unwrap();
    }
}

// Local Variables:
// flycheck-rust-crate-root: "lib.rs"
// End:
