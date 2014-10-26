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
use std::ptr;

use super::SqliteError;
use core::{Access, DatabaseConnection};
use consts::{OpenFlags};
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
pub fn open(filename: &str, flags: OpenFlags) -> Result<DatabaseConnection, SqliteError> {
    DatabaseConnection::new(ByFilename { filename: filename, flags: flags })
}

/// Access to a database by filename
pub struct ByFilename<'a> {
    /// Filename or sqlite3 style URI.
    pub filename: &'a str,
    /// Flags for additional control over the new database connection.
    pub flags: OpenFlags
}

impl<'a> Access for ByFilename<'a> {
    fn open(self, db: *mut *mut ffi::sqlite3) -> c_int {
        self.filename.with_c_str({
            |filename| unsafe { ffi::sqlite3_open_v2(filename, db, self.flags.bits(), ptr::null()) }
        })
    }
}



#[cfg(test)]
mod tests {
    use super::ByFilename;
    use core::DatabaseConnection;
    use consts;


    #[test]
    fn open_file_db() {
        DatabaseConnection::new(ByFilename { filename: "/tmp/db1", flags: consts::DEFAULT_OPEN_FLAGS }).unwrap();
    }
}

// Local Variables:
// flycheck-rust-crate-root: "lib.rs"
// End:
