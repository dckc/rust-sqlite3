#![crate_name = "sqlite3"]
#![crate_type = "lib"]
#![feature(unsafe_destructor)]

extern crate libc;

use libc::c_int;
use std::num::from_uint;
use std::ptr;
use std::c_str;

mod safe;
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
        let mut db = ptr::mut_null();
        let result = ":memory:".with_c_str({
            |memory|
            unsafe { ffi::sqlite3_open(memory, &mut db) }
        });
        match decode_result(result, "sqlite3_open") {
            Ok(()) => Ok(SqliteConnection { db: db }),
            Err(err) => {
                // "Whether or not an error occurs when it is opened,
                // resources associated with the database connection
                // handle should be released by passing it to
                // sqlite3_close() when it is no longer required."
                unsafe { ffi::sqlite3_close_v2(db) };
                Err(err)
            }
        }
    }

    /// Prepare/compile an SQL statement.
    /// See http://www.sqlite.org/c3ref/prepare.html
    pub fn prepare<'db>(&'db mut self, sql: &str) -> SqliteResult<SqliteStatement<'db>> {
        match self.prepare_with_offset(sql) {
            Ok((cur, _)) => Ok(cur),
            Err(e) => Err(e)
        }
    }
                
    pub fn prepare_with_offset<'db>(&'db mut self, sql: &str) -> SqliteResult<(SqliteStatement<'db>, uint)> {
        let mut stmt = ptr::mut_null();
        let mut tail = ptr::null();
        let z_sql = sql.as_ptr() as *const ::libc::c_char;
        let n_byte = sql.len() as c_int;
        let r = unsafe { ffi::sqlite3_prepare_v2(self.db, z_sql, n_byte, &mut stmt, &mut tail) };
        match decode_result(r, "sqlite3_prepare_v2") {
            Ok(()) => {
                let offset = tail as uint - z_sql as uint;
                Ok((SqliteStatement::new(stmt), offset))
            },
            Err(code) => Err(code)
        }
    }

}


pub struct SqliteStatement<'db> {
    stmt: *mut ffi::sqlite3_stmt
}

#[unsafe_destructor]
impl<'db> Drop for SqliteStatement<'db> {
    fn drop(&mut self) {
        unsafe {

            // We ignore the return code from finalize because:

            // "If If the most recent evaluation of statement S
            // failed, then sqlite3_finalize(S) returns the
            // appropriate error codethe most recent evaluation of
            // statement S failed, then sqlite3_finalize(S) returns
            // the appropriate error code"

            // "The sqlite3_finalize(S) routine can be called at any
            // point during the life cycle of prepared statement S"

            ffi::sqlite3_finalize(self.stmt);
        }
    }
}


impl<'db> SqliteStatement<'db> {
    // Only a SqliteCursor can call this constructor
    #[allow(visible_private_types)]
    pub fn new<'db>(stmt: *mut ffi::sqlite3_stmt) -> SqliteStatement<'db> {
        SqliteStatement { stmt: stmt }
    }

    pub fn query(&mut self) -> SqliteResult<SqliteRows> {
        {
            let r = unsafe { ffi::sqlite3_reset(self.stmt) };
            try!(decode_result(r, "sqlite3_reset"))
        }
        Ok(SqliteRows::new(self))
    }
}


pub struct SqliteRows<'s> {
    statement: &'s mut SqliteStatement<'s>,
}

impl<'s> SqliteRows<'s> {
    pub fn new(statement: &'s mut SqliteStatement) -> SqliteRows<'s> {
        SqliteRows { statement: statement }
    }
}

impl<'s> SqliteRows<'s> {
    // An sqlite "row" only lasts until the next call to step(),
    // so this can't match the Iterator trait.
    pub fn next<'r>(&'r mut self) -> Option<SqliteResult<SqliteRow<'s, 'r>>> {
        let result = unsafe { ffi::sqlite3_step(self.statement.stmt) } as uint;
        match from_uint::<SqliteStep>(result) {
            Some(SQLITE_ROW) => {
                Some(Ok(SqliteRow{ rows: self }))
            },
            Some(SQLITE_DONE) => None,
            None => {
                let err = from_uint::<SqliteError>(result);
                Some(Err(err.unwrap()))
            }
        }
    }
}


pub struct SqliteRow<'s, 'r> {
    rows: &'r mut SqliteRows<'s>
}

impl<'s, 'r> SqliteRow<'s, 'r> {

    // TODO: consider returning Option<uint>
    // "This routine returns 0 if pStmt is an SQL statement that does
    // not return data (for example an UPDATE)."
    pub fn column_count(&self) -> uint {
        let stmt = self.rows.statement.stmt;
        let result = unsafe { ffi::sqlite3_column_count(stmt) };
        result as uint
    }

    // See http://www.sqlite.org/c3ref/column_name.html
    pub fn with_column_name<T>(&mut self, i: uint, default: T, f: |&str| -> T) -> T {
        let stmt = self.rows.statement.stmt;
        let n = i as c_int;
        let result = unsafe { ffi::sqlite3_column_name(stmt, n) };
        if result == ptr::null() { default }
        else {
            let name = unsafe { c_str::CString::new(result, false) };
            match name.as_str() {
                Some(name) => f(name),
                None => default
            }
        }
    }

    pub fn column_int(&self, col: uint) -> i32 {
        let stmt = self.rows.statement.stmt;
        let i_col = col as c_int;
        unsafe { ffi::sqlite3_column_int(stmt, i_col) }
    }
}

// ref http://www.sqlite.org/c3ref/c_abort.html
#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
enum SqliteOk {
    SQLITE_OK = 0
}


#[must_use]
pub type SqliteResult<T> = Result<T, SqliteError>;

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
pub fn decode_result(result: c_int, context: &str) -> SqliteResult<()> {
    if result == SQLITE_OK as c_int {
        Ok(())
    } else {
        match from_uint::<SqliteError>(result as uint) {
            Some(code) => Err(code),
            None => fail!("{} returned unexpected {:d}", context, result)
        }
    }
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
    use super::{SqliteConnection, SqliteResult, SqliteRows};

    #[test]
    fn db_new_types() {
        SqliteConnection::new().unwrap();
    }

    #[test]
    fn stmt_new_types() {
        fn go() -> SqliteResult<()> {
            let mut db = try!(SqliteConnection::new());
            db.prepare("select 1 + 1").map( |_s| () )
        }
        go().unwrap();
    }


    fn with_query<T>(sql: &str, f: |rows: &mut SqliteRows| -> T) -> SqliteResult<T> {
        let mut db = try!(SqliteConnection::new());
        let mut s = try!(db.prepare(sql));
        let mut rows = try!(s.query());
        Ok(f(&mut rows))
    }

    #[test]
    fn query_two_rows() {
        fn go() -> SqliteResult<(uint, i32)> {
            let mut count = 0;
            let mut sum = 0;

            with_query("select 1
                       union all
                       select 2", |rows| {
                loop {
                    match rows.next() {
                        Some(Ok(ref mut row)) => {
                            count += 1;
                            sum += row.get(0u)
                        },
                        _ => break
                    }
                }
                (count, sum)
            })
        }
        assert_eq!(go(), Ok((2, 3)))
    }

    #[test]
    fn named_rowindex() {
        fn go() -> SqliteResult<(uint, i32)> {
            let mut count = 0;
            let mut sum = 0;

            with_query("select 1 as col1
                       union all
                       select 2", |rows| {
                loop {
                    match rows.next() {
                        Some(Ok(ref mut row)) => {
                            count += 1;
                            sum += row.get("col1")
                        },
                        _ => break
                    }
                }
                (count, sum)
            })
        }
        assert_eq!(go(), Ok((2, 3)))
    }
}
