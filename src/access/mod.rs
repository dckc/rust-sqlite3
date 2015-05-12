//! Access to open sqlite3 database by filename.
//!
//! The `core` module requires explicit authority to access files and such,
//! following the principle of least authority.
//!
//! This module provides the privileged functions to create such authorities.
//!
//! *TODO: move `mod access` to its own crate so that linking to `sqlite3` doesn't
//! bring in this ambient authority.*

use libc::c_int;
use std::ptr;

use super::SqliteResult;
use core::{Access, DatabaseConnection, str_charstar};
use ffi;

use access::flags::OpenFlags;

// submodule KLUDGE around missing_docs for bitflags!()
#[allow(missing_docs)]
pub mod flags;

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
pub fn open(filename: &str, flags: Option<OpenFlags>) -> SqliteResult<DatabaseConnection> {
    DatabaseConnection::new(
        ByFilename {
            filename: filename,
            flags: flags.unwrap_or_default()
        })
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
        let c_filename = str_charstar(self.filename).as_ptr();
        let flags = self.flags.bits();
        unsafe { ffi::sqlite3_open_v2(c_filename, db, flags, ptr::null()) }
    }
}



#[cfg(test)]
mod tests {
    use std::default::Default;
    use super::ByFilename;
    use core::DatabaseConnection;
    use std::env::temp_dir;

    #[test]
    fn open_file_db() {
        let mut temp_directory = temp_dir();
        temp_directory.push("db1");
        let path = temp_directory.into_os_string().into_string().unwrap();
        DatabaseConnection::new(
            ByFilename {
                filename: path.as_ref(), flags: Default::default()
            })
            .unwrap();
    }
}

// Local Variables:
// flycheck-rust-crate-root: "lib.rs"
// End:
