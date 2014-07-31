#![crate_name = "sqlite3"]
#![crate_type = "lib"]

extern crate libc;

use libc::c_int;
use std::num::from_uint;
use std::ptr;

#[allow(non_camel_case_types, dead_code)]
mod ffi;

pub struct SqliteConnection {
    // not pub so that nothing outside this module
    // interferes with the lifetime
    db: *mut ffi::sqlite3
}

impl Drop for SqliteConnection {
    fn drop(&mut self) {
        let ok = unsafe { ffi::sqlite3_close_v2(self.db) };
        assert_eq!(ok, SQLITE_OK as c_int);
    }
}


impl SqliteConnection {
    // Create a new connection to an in-memory database.
    // TODO: explicit access to files
    // TODO: use support _v2 interface with flags
    // TODO: integrate sqlite3_errmsg()
    pub fn new() -> Result<SqliteConnection, SqliteError> {
        let memory = ":memory:".as_ptr() as *const ::libc::c_char;
        let mut db = ptr::mut_null::<ffi::sqlite3>();
        let result = unsafe { ffi::sqlite3_open(memory, &mut db) };
        match result {
            ok if ok == SQLITE_OK as c_int => Ok(SqliteConnection { db: db }),
            err => {
                unsafe { ffi::sqlite3_close_v2(db) };
                Err(decode_error(err).unwrap())
            }
        }
    }
}

// ref http://www.sqlite.org/c3ref/c_abort.html
#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
enum SqliteOk {
    SQLITE_OK = 0
}


#[must_use]
type SqliteResult<T> = Result<T, SqliteError>;

#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
pub enum SqliteError {
    SQLITE_ERROR     =  1,
    SQLITE_INTERNAL  =  2,
    SQLITE_PERM      =  3,
    SQLITE_ABORT     =  4,
    SQLITE_BUSY      =  5,
    SQLITE_LOCKED    =  6,
    SQLITE_NOMEM     =  7,
    SQLITE_READONLY  =  8,
    SQLITE_INTERRUPT =  9,
    SQLITE_IOERR     = 10,
    SQLITE_CORRUPT   = 11,
    SQLITE_NOTFOUND  = 12,
    SQLITE_FULL      = 13,
    SQLITE_CANTOPEN  = 14,
    SQLITE_PROTOCOL  = 15,
    SQLITE_EMPTY     = 16,
    SQLITE_SCHEMA    = 17,
    SQLITE_TOOBIG    = 18,
    SQLITE_CONSTRAINT= 19,
    SQLITE_MISMATCH  = 20,
    SQLITE_MISUSE    = 21,
    SQLITE_NOLFS     = 22,
    SQLITE_AUTH      = 23,
    SQLITE_FORMAT    = 24,
    SQLITE_RANGE     = 25,
    SQLITE_NOTADB    = 26
}

#[inline]
pub fn decode_error(err: c_int) -> Option<SqliteError> {
    from_uint::<SqliteError>(err as uint)
}

#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
pub enum SqliteLogLevel {
    SQLITE_NOTICE    = 27,
    SQLITE_WARNING   = 28,
}

#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
enum SqliteStep {
    SQLITE_ROW       = 100,
    SQLITE_DONE      = 101,
}


#[cfg(test)]
mod tests {
    use super::SqliteConnection;

    #[test]
    fn db_new_types() {
        SqliteConnection::new().unwrap();
    }
}
