//! A minimal safe interface to sqlite3's basic API.
//!
//! The basic sqlite3 API is discussed in the [sqlite intro][intro].
//! To go beyond that, use the (unsafe) `ffi` module directly.
//!
//! [intro]: http://www.sqlite.org/cintro.html
//!
//! ```rust
//! extern crate sqlite3;
//!
//! use sqlite3::{
//!     DatabaseConnection,
//!     SqliteResult,
//! };
//!
//! fn convenience_exec() -> SqliteResult<DatabaseConnection> {
//!     let mut conn = try!(DatabaseConnection::in_memory());
//!
//!     try!(conn.exec("
//!        create table items (
//!                    id integer,
//!                    description varchar(40),
//!                    price integer
//!                    )"));
//!
//!     Ok(conn)
//! }
//!
//! fn typical_usage(conn: &mut DatabaseConnection) -> SqliteResult<String> {
//!     {
//!         let mut stmt = try!(conn.prepare(
//!             "insert into items (id, description, price)
//!            values (1, 'stuff', 10)"));
//!         let mut results = stmt.execute();
//!         match try!(results.step()) {
//!             None => (),
//!             Some(_) => panic!("row from insert?!")
//!         };
//!     }
//!     assert_eq!(conn.changes(), 1);
//!     assert_eq!(conn.last_insert_rowid(), 1);
//!     {
//!         let mut stmt = try!(conn.prepare(
//!             "select * from items"));
//!         let mut results = stmt.execute();
//!         match results.step() {
//!             Ok(Some(ref mut row1)) => {
//!                 let id = row1.column_int(0);
//!                 let desc_opt = row1.column_text(1).expect("desc_opt should be non-null");
//!                 let price = row1.column_int(2);
//!
//!                 assert_eq!(id, 1);
//!                 assert_eq!(desc_opt, format!("stuff"));
//!                 assert_eq!(price, 10);
//!
//!                 Ok(format!("row: {}, {}, {}", id, desc_opt, price))
//!             },
//!             Err(oops) => panic!(oops),
//!             Ok(None) => panic!("where did our row go?")
//!         }
//!     }
//! }
//!
//! pub fn main() {
//!     match convenience_exec() {
//!         Ok(ref mut db) => {
//!             match typical_usage(db) {
//!                 Ok(txt) => println!("item: {}", txt),
//!                 Err(oops) => {
//!                     panic!("error: {:?} msg: {}", oops,
//!                            db.errmsg())
//!                 }
//!             }
//!         },
//!         Err(oops) => panic!(oops)
//!     }
//! }
//! ```
//!
//! The `DatabaseConnection` and `PreparedStatment` structures are
//! memory-safe versions of the sqlite3 connection and prepared
//! statement structures. A `PreparedStatement` maintains mutable,
//! and hence exclusive, reference to the database connection.
//! Note the use of blocks avoid borrowing the connection more
//! than once at a time.
//!
//! In addition:
//!
//!   - `ResultSet` represents, as a rust lifetime, all of the steps
//!     of one execution of a statement. (*Ideally, it would be an
//!     Iterator over `ResultRow`s, but the `Iterator::next()`
//!     function has no lifetime parameter.*) Use of mutable
//!     references ensures that its lifetime is subsumed by the
//!     statement lifetime.  Its destructor resets the statement.
//!
//!   - `ResultRow` is a lifetime for access to the columns of one row.
//!

use libc::{c_int, c_char};
use std::ffi as std_ffi;
use std::mem;
use std::num::from_i32;
use std::ptr;
use std::str;
use std::time::Duration;
use std::marker::PhantomData;
use std::ffi::CStr;

use self::SqliteOk::SQLITE_OK;
use self::Step::{SQLITE_ROW, SQLITE_DONE};

pub use super::{
    SqliteError,
    SqliteErrorCode,
    SqliteResult,
};

pub use super::ColumnType;
pub use super::ColumnType::SQLITE_NULL;

use ffi; // TODO: move to sqlite3-sys crate


/// Successful result
///
/// Use `SQLITE_OK as c_int` to decode return values from mod ffi.
/// See SqliteResult, SqliteError for typical return code handling.
#[derive(Debug, PartialEq, Eq, FromPrimitive, Copy, Clone)]
#[allow(non_camel_case_types)]
#[allow(missing_docs)]
pub enum SqliteOk {
    SQLITE_OK = 0
}


#[derive(Debug, PartialEq, Eq, FromPrimitive, Copy, Clone)]
#[allow(non_camel_case_types)]
// TODO: use, test this
enum SqliteLogLevel {
    SQLITE_NOTICE    = 27,
    SQLITE_WARNING   = 28,
}

/// A connection to a sqlite3 database.
pub struct DatabaseConnection {
    // not pub so that nothing outside this module
    // interferes with the lifetime
    db: *mut ffi::sqlite3,

    // whether to copy errmsg() to error detail
    detailed: bool
}

impl Drop for DatabaseConnection {
    /// Release resources associated with connection.
    ///
    /// # Failure
    ///
    /// Fails if "the database connection is associated with
    /// unfinalized prepared statements or unfinished sqlite3_backup
    /// objects"[1] which the Rust memory model ensures is impossible
    /// (barring bugs in the use of unsafe blocks in the implementation
    /// of this library).
    ///
    /// [1]: http://www.sqlite.org/c3ref/close.html
    fn drop(&mut self) {
        // sqlite3_close_v2 is for gced languages.
        let ok = unsafe { ffi::sqlite3_close(self.db) };
        assert_eq!(ok, SQLITE_OK as c_int);
    }
}


/// Authorization to connect to database.
pub trait Access {
    /// Open a database connection.
    ///
    /// Whether or not an error occurs, allocate a handle and update
    /// db to point to it.  return `SQLITE_OK as c_int` or set the
    /// `errmsg` of the db handle and return a relevant result code.
    fn open(self, db: *mut *mut ffi::sqlite3) -> c_int;
}


// why isn't this in std::option?
fn maybe<T>(choice: bool, x: T) -> Option<T> {
    if choice { Some(x) } else { None }
}

use std::convert::From;
use std::ffi::NulError;
impl From<NulError> for SqliteError {
    fn from(_: NulError) -> SqliteError {
        SqliteError{
            kind: SqliteErrorCode::SQLITE_MISUSE,
            desc: "Sql string contained an internal 0 byte",
            detail: None
        }
    }
}

impl DatabaseConnection {
    /// Given explicit access to a database, attempt to connect to it.
    ///
    /// Note `SqliteError` code is accompanied by (copy) of `sqlite3_errmsg()`.
    pub fn new<A: Access>(access: A) -> SqliteResult<DatabaseConnection> {
        let mut db = ptr::null_mut();
        let result = access.open(&mut db);
        match decode_result(result, "sqlite3_open_v2", Some(db)) {
            Ok(()) => Ok(DatabaseConnection { db: db, detailed: true }),
            Err(err) => {
                // "Whether or not an error occurs when it is opened,
                // resources associated with the database connection
                // handle should be released by passing it to
                // sqlite3_close() when it is no longer required."
                unsafe { ffi::sqlite3_close(db) };

                Err(err)
            }
        }
    }

    /// Opt out of copies of error message details.
    pub fn ignore_detail(&mut self) {
        self.detailed = false;
    }


    /// Create connection to an in-memory database.
    ///
    ///  - TODO: integrate sqlite3_errmsg()
    #[unstable]
    pub fn in_memory() -> SqliteResult<DatabaseConnection> {
        struct InMemory;
        impl Access for InMemory {
            fn open(self, db: *mut *mut ffi::sqlite3) -> c_int {
                let c_memory = str_charstar(":memory:").as_ptr();
                unsafe { ffi::sqlite3_open(c_memory, db) }
            }
        }
        DatabaseConnection::new(InMemory)
    }

    /// Prepare/compile an SQL statement.
    pub fn prepare<'db:'st, 'st>(&'db self, sql: &str) -> SqliteResult<PreparedStatement<'st>> {
        match self.prepare_with_offset(sql) {
            Ok((cur, _)) => Ok(cur),
            Err(e) => Err(e)
        }
    }

    /// Prepare/compile an SQL statement and give offset to remaining text.
    ///
    /// *TODO: give caller a safe way to use the offset. Perhaps
    /// return a &'x str?*
    #[unstable]
    pub fn prepare_with_offset<'db:'st, 'st>(&'db self, sql: &str)
                                    -> SqliteResult<(PreparedStatement<'st>, usize)> {
        let mut stmt = ptr::null_mut();
        let mut tail = ptr::null();
        let z_sql = str_charstar(sql).as_ptr();
        let n_byte = sql.len() as c_int;
        let r = unsafe { ffi::sqlite3_prepare_v2(self.db, z_sql, n_byte, &mut stmt, &mut tail) };
        match decode_result(r, "sqlite3_prepare_v2", maybe(self.detailed, self.db)) {
            Ok(()) => {
                let offset = tail as usize - z_sql as usize;
                Ok((PreparedStatement { stmt: stmt , detailed: self.detailed, marker: PhantomData }, offset))
            },
            Err(code) => Err(code)
        }
    }

    /// Return a copy of the latest error message.
    ///
    /// Return `""` in case of ill-formed utf-8 or null.
    ///
    /// *TODO: represent error state in types: "If a prior API call
    /// failed but the most recent API call succeeded, the return
    /// value from sqlite3_errcode() is undefined."*
    ///
    /// cf `ffi::sqlite3_errmsg`.
    #[unstable]
    pub fn errmsg(&mut self) -> String {
        DatabaseConnection::_errmsg(self.db)
    }

    fn _errmsg(db: *mut ffi::sqlite3) -> String {
        let errmsg = unsafe { ffi::sqlite3_errmsg(db) };
        // returning Option<String> doesn't seem worthwhile.
        charstar_str(&(errmsg)).unwrap_or("").to_string()
    }

    /// One-Step Query Execution Interface
    ///
    /// cf [sqlite3_exec][exec]
    /// [exec]: http://www.sqlite.org/c3ref/exec.html
    ///
    ///  - TODO: callback support?
    ///  - TODO: errmsg support
    #[unstable]
    pub fn exec(&mut self, sql: &str) -> SqliteResult<()> {
        let db = self.db;
        let c_sql = try!(std_ffi::CString::new(sql.as_bytes()));
        let result = unsafe {
            ffi::sqlite3_exec(db, c_sql.as_ptr(), None,
                              ptr::null_mut(), ptr::null_mut())
        };
        decode_result(result, "sqlite3_exec", maybe(self.detailed, self.db))
    }

    /// Return the number of database rows that were changed or
    /// inserted or deleted by the most recently completed SQL
    /// statement.
    ///
    /// cf `sqlite3_changes`.
    pub fn changes(&self) -> u64 {
        let db = self.db;
        let count = unsafe { ffi::sqlite3_changes(db) };
        count as u64
    }

    /// Set a busy timeout and clear any previously set handler.
    /// If duration is zero or negative, turns off busy handler.
    pub fn busy_timeout(&mut self, d: Duration) -> SqliteResult<()> {
        let ms = d.num_milliseconds() as i32;
        let result = unsafe { ffi::sqlite3_busy_timeout(self.db, ms) };
        decode_result(result, "sqlite3_busy_timeout", maybe(self.detailed, self.db))
    }

    /// Return the rowid of the most recent successful INSERT into
    /// a rowid table or virtual table.
    ///
    /// cf `sqlite3_last_insert_rowid`
    pub fn last_insert_rowid(&self) -> i64 {
        unsafe { ffi::sqlite3_last_insert_rowid(self.db) }
    }

    /// Expose the underlying `sqlite3` struct pointer for use
    /// with the `ffi` module.
    pub unsafe fn expose(&mut self) -> *mut ffi::sqlite3 {
        self.db
    }
}


/// Convert from sqlite3 API utf8 to rust str.
fn charstar_str<'a>(utf_bytes: &'a *const c_char) -> Option<&'a str> {
    if *utf_bytes == ptr::null() {
        return None;
    }
    let c_str = unsafe { CStr::from_ptr(*utf_bytes) };

    Some( unsafe { str::from_utf8_unchecked(c_str.to_bytes()) } )
}

/// Convenience function to get a CString from a str
#[inline(always)]
pub fn str_charstar<'a>(s: &'a str) -> std_ffi::CString {
    std_ffi::CString::new(s.as_bytes()).unwrap_or(std_ffi::CString::new("").unwrap())
}

/// A prepared statement.
pub struct PreparedStatement<'st> {
    stmt: *mut ffi::sqlite3_stmt,
    detailed: bool,
    marker: PhantomData<&'st DatabaseConnection>,
}

#[unsafe_destructor]
impl<'st> Drop for PreparedStatement<'st> {
    fn drop(&mut self) {
        unsafe {

            // We ignore the return code from finalize because:

            // "If If the most recent evaluation of statement S
            // failed, then sqlite3_finalize(S) returns the
            // appropriate error code"

            // "The sqlite3_finalize(S) routine can be called at any
            // point during the life cycle of prepared statement S"

            ffi::sqlite3_finalize(self.stmt);
        }
    }
}


/// Type for picking out a bind parameter.
/// 1-indexed
pub type ParamIx = u16;

impl<'st:'res, 'res> PreparedStatement<'st> {
    /// Begin executing a statement.
    pub fn execute(&'res mut self) -> ResultSet<'st, 'res> {
        ResultSet { statement: self }
    }
}

/// A compiled prepared statement that may take parameters.
/// **Note:** "The leftmost SQL parameter has an index of 1."[1]
///
/// [1]: http://www.sqlite.org/c3ref/bind_blob.html
impl<'st> PreparedStatement<'st> {

    /// Opt out of copies of error message details.
    pub fn ignore_detail(&mut self) {
        self.detailed = false;
    }


    fn detail_db(&mut self) -> Option<*mut ffi::sqlite3> {
        if self.detailed {
            let db = unsafe { ffi::sqlite3_db_handle(self.stmt) };
            Some(db)
        } else {
            None
        }
    }

    fn get_detail(&mut self) -> Option<String> {
        self.detail_db().map(|db| DatabaseConnection::_errmsg(db))
    }

    /// Bind null to a statement parameter.
    pub fn bind_null(&mut self, i: ParamIx) -> SqliteResult<()> {
        let ix = i as c_int;
        let r = unsafe { ffi::sqlite3_bind_null(self.stmt, ix ) };
        decode_result(r, "sqlite3_bind_null", self.detail_db())
    }

    /// Bind an int to a statement parameter.
    pub fn bind_int(&mut self, i: ParamIx, value: i32) -> SqliteResult<()> {
        let ix = i as c_int;
        let r = unsafe { ffi::sqlite3_bind_int(self.stmt, ix, value) };
        decode_result(r, "sqlite3_bind_int", self.detail_db())
    }

    /// Bind an int64 to a statement parameter.
    pub fn bind_int64(&mut self, i: ParamIx, value: i64) -> SqliteResult<()> {
        let ix = i as c_int;
        let r = unsafe { ffi::sqlite3_bind_int64(self.stmt, ix, value) };
        decode_result(r, "sqlite3_bind_int64", self.detail_db())
    }

    /// Bind a double to a statement parameter.
    pub fn bind_double(&mut self, i: ParamIx, value: f64) -> SqliteResult<()> {
        let ix = i as c_int;
        let r = unsafe { ffi::sqlite3_bind_double(self.stmt, ix, value) };
        decode_result(r, "sqlite3_bind_double", self.detail_db())
    }

    /// Bind a (copy of a) str to a statement parameter.
    ///
    /// *TODO: support binding without copying strings, blobs*
    #[unstable]
    pub fn bind_text(&mut self, i: ParamIx, value: &str) -> SqliteResult<()> {
        let ix = i as c_int;
        // SQLITE_TRANSIENT => SQLite makes a copy
        let transient = unsafe { mem::transmute(-1 as isize) };
        let c_value = str_charstar(value).as_ptr();
        let len = value.len() as c_int;
        let r = unsafe { ffi::sqlite3_bind_text(self.stmt, ix, c_value, len, transient) };
        decode_result(r, "sqlite3_bind_text", self.detail_db())
    }

    /// Bind a (copy of a) byte sequence to a statement parameter.
    ///
    /// *TODO: support binding without copying strings, blobs*
    #[unstable]
    pub fn bind_blob(&mut self, i: ParamIx, value: &[u8]) -> SqliteResult<()> {
        let ix = i as c_int;
        // SQLITE_TRANSIENT => SQLite makes a copy
        let transient = unsafe { mem::transmute(-1 as isize) };
        let len = value.len() as c_int;
        // from &[u8] to &[i8]
        let val = unsafe { mem::transmute(value.as_ptr()) };
        let r = unsafe { ffi::sqlite3_bind_blob(self.stmt, ix, val, len, transient) };
        decode_result(r, "sqlite3_bind_blob", self.detail_db())
    }

    /// Clear all parameter bindings.
    pub fn clear_bindings(&'st mut self) {
        // We ignore the return value, since no return codes are documented.
        unsafe { ffi::sqlite3_clear_bindings(self.stmt) };
    }

    /// Return the number of SQL parameters.
    /// If parameters of the ?NNN form are used, there may be gaps in the list.
    pub fn bind_parameter_count(&mut self) -> ParamIx {
        let count = unsafe { ffi::sqlite3_bind_parameter_count(self.stmt) };
        count as ParamIx
    }

    /// Expose the underlying `sqlite3_stmt` struct pointer for use
    /// with the `ffi` module.
    pub unsafe fn expose(&mut self) -> *mut ffi::sqlite3_stmt {
        self.stmt
    }
}


/// Results of executing a `prepare()`d statement.
pub struct ResultSet<'st:'res, 'res> {
    statement: &'res mut PreparedStatement<'st>,
}

#[derive(Debug, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
enum Step {
    SQLITE_ROW       = 100,
    SQLITE_DONE      = 101,
}


#[unsafe_destructor]
impl<'st, 'res> Drop for ResultSet<'st, 'res> {
    fn drop(&mut self) {

        // We ignore the return code from reset because it has already
        // been reported:
        //
        // "If the most recent call to sqlite3_step(S) for the prepared
        // statement S indicated an error, then sqlite3_reset(S)
        // returns an appropriate error code."
        unsafe { ffi::sqlite3_reset(self.statement.stmt) };
    }
}


impl<'st:'res, 'res:'row, 'row> ResultSet<'st, 'res> {
    /// Execute the next step of a prepared statement.
    ///
    /// An sqlite "row" only lasts until the next call to `ffi::sqlite3_step()`,
    /// so we need a lifetime constraint. The unfortunate result is that
    ///  `ResultSet` cannot implement the `Iterator` trait.
    pub fn step(&'row mut self) -> SqliteResult<Option<ResultRow<'st, 'res, 'row>>> {
        let result = unsafe { ffi::sqlite3_step(self.statement.stmt) };
        match from_i32::<Step>(result) {
            Some(SQLITE_ROW) => {
                Ok(Some(ResultRow{ rows: self }))
            },
            Some(SQLITE_DONE) => Ok(None),
            None => Err(error_result(result, "step", self.statement.get_detail()))
        }
    }
}


/// Access to columns of a row.
pub struct ResultRow<'st:'res, 'res:'row, 'row> {
    rows: &'row mut ResultSet<'st, 'res>
}

/// Column index for accessing parts of a row.
pub type ColIx = u32;

/// Access to one row (step) of a result.
///
/// Note "These routines attempt to convert the value where appropriate."[1]
/// and "The value returned by `sqlite3_column_type()` is only
/// meaningful if no type conversions have occurred as described
/// below. After a type conversion, the value returned by
/// `sqlite3_column_type()` is undefined."[1]
///
/// [1]: http://www.sqlite.org/c3ref/column_blob.html
impl<'st, 'res, 'row> ResultRow<'st, 'res, 'row> {

    /// cf `sqlite3_column_count`
    ///
    /// *TODO: consider returning Option<uint>
    /// "This routine returns 0 if pStmt is an SQL statement that does
    /// not return data (for example an UPDATE)."*
    #[unstable]
    pub fn column_count(&self) -> ColIx {
        let stmt = self.rows.statement.stmt;
        let result = unsafe { ffi::sqlite3_column_count(stmt) };
        result as ColIx
    }

    /// Look up a column name and compute some function of it.
    ///
    /// Return `default` if there is no column `i`
    ///
    /// cf `sqlite_column_name`
    pub fn with_column_name<T, F: Fn(&str) -> T>(&mut self, i: ColIx, default: T, f: F) -> T {
        let stmt = self.rows.statement.stmt;
        let n = i as c_int;
        let result = unsafe { ffi::sqlite3_column_name(stmt, n) };
        match charstar_str(&result) {
            Some(name) => f(name),
            None => default
        }
    }

    /// Look up the type of a column.
    ///
    /// Return `SQLITE_NULL` if there is no such `col`.
    pub fn column_type(&self, col: ColIx) -> ColumnType {
        let stmt = self.rows.statement.stmt;
        let i_col = col as c_int;
        let result = unsafe { ffi::sqlite3_column_type(stmt, i_col) };
        // fail on out-of-range result instead?
        from_i32::<ColumnType>(result).unwrap_or(SQLITE_NULL)
    }

    /// Get `int` value of a column.
    pub fn column_int(&self, col: ColIx) -> i32 {
        let stmt = self.rows.statement.stmt;
        let i_col = col as c_int;
        unsafe { ffi::sqlite3_column_int(stmt, i_col) }
    }

    /// Get `int64` value of a column.
    pub fn column_int64(&self, col: ColIx) -> i64 {
        let stmt = self.rows.statement.stmt;
        let i_col = col as c_int;
        unsafe { ffi::sqlite3_column_int64(stmt, i_col) }
    }

    /// Get `f64` (aka double) value of a column.
    pub fn column_double(&self, col: ColIx) -> f64 {
        let stmt = self.rows.statement.stmt;
        let i_col = col as c_int;
        unsafe { ffi::sqlite3_column_double(stmt, i_col) }
    }

    /// Get `Option<String>` (aka text) value of a column.
    pub fn column_text(&mut self, col: ColIx) -> Option<String> {
        let stmt = self.rows.statement.stmt;
        let i_col = col as c_int;
        let s = unsafe { ffi::sqlite3_column_text(stmt, i_col) };
        charstar_str(&(s as *const c_char)).map(|f: &str| { f.to_string() })
    }

    /// Get `Option<Vec<u8>>` (aka blob) value of a column.
    pub fn column_blob(&mut self, col: ColIx) -> Option<Vec<u8>> {
        let stmt = self.rows.statement.stmt;
        let i_col = col as c_int;
        let bs = unsafe { ffi::sqlite3_column_blob(stmt, i_col) } as *const ::libc::c_uchar;
        if bs == ptr::null() {
            return None;
        }
        let len = unsafe { ffi::sqlite3_column_bytes(stmt, i_col) };
        Some(unsafe { Vec::from_raw_buf(bs, len as usize)} )
    }

}


/// Decode SQLite result as `SqliteResult`.
///
/// Note the use of the `Result<T, E>` pattern to distinguish errors in
/// the type system.
///
/// # Panic
///
/// Panics if result is not a SQLITE error code.
pub fn decode_result(
    result: c_int,
    desc: &'static str,
    detail_db: Option<*mut ffi::sqlite3>,
    ) -> SqliteResult<()> {
    if result == SQLITE_OK as c_int {
        Ok(())
    } else {
        let detail = detail_db.map(|db| DatabaseConnection::_errmsg(db));
        Err(error_result(result, desc, detail))
    }
}


fn error_result(
    result: c_int,
    desc: &'static str,
    detail: Option<String>
    ) -> SqliteError {
    SqliteError {
        kind: from_i32::<SqliteErrorCode>(result).unwrap(),
        desc: desc,
        detail: detail
    }
}


#[cfg(test)]
mod test_opening {
    use super::{DatabaseConnection, SqliteResult};
    use std::time::Duration;

    #[test]
    fn db_construct_typechecks() {
        assert!(DatabaseConnection::in_memory().is_ok())
    }

    #[test]
    fn db_busy_timeout() {
        fn go() -> SqliteResult<()> {
            let mut db = try!(DatabaseConnection::in_memory());
            db.busy_timeout(Duration::seconds(2))
        }
        go().unwrap();
    }

    // TODO: _v2 with flags
}


#[cfg(test)]
mod tests {
    use super::{DatabaseConnection, SqliteResult, ResultSet};
    use super::super::{ResultRowAccess};

    #[test]
    fn stmt_new_types() {
        fn go() -> SqliteResult<()> {
            let db = try!(DatabaseConnection::in_memory());
            let res = db.prepare("select 1 + 1").map( |_s| () );
            res
        }
        go().unwrap();
    }


    fn with_query<T, F>(sql: &str, mut f: F) -> SqliteResult<T>
        where F: FnMut(&mut ResultSet) -> T
    {
        let db = try!(DatabaseConnection::in_memory());
        let mut s = try!(db.prepare(sql));
        let mut rows = s.execute();
        Ok(f(&mut rows))
    }

    #[test]
    fn query_two_rows() {
        fn go() -> SqliteResult<(u32, i32)> {
            let mut count = 0;
            let mut sum:i32 = 0;

            with_query("select 1
                       union all
                       select 2", |rows| {
                loop {
                    match rows.step() {
                        Ok(Some(mut row)) => {
                            count += 1;
                            let result :i32 = row.get(0);
                            sum += result;
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
    fn query_null_string() {
        with_query("select null", |rows| {
            match rows.step() {
                Ok(Some(ref mut row)) => {
                    assert_eq!(row.column_text(0), None);
                }
                _ => { panic!("Expected a row"); }
            }
        }).unwrap();
    }

    #[test]
    fn detailed_errors() {
        let go = || -> SqliteResult<()> {
            let db = try!(DatabaseConnection::in_memory());
            try!(db.prepare("select bogus"));
            Ok( () )
        };
        let err = go().err().unwrap();
        assert_eq!(err.detail(), Some("no such column: bogus".to_string()))
    }

    #[test]
    fn no_alloc_errors_db() {
        let go = || {
            let mut db = try!(DatabaseConnection::in_memory());
            db.ignore_detail();
            try!(db.prepare("select bogus"));
            Ok( () )
        };
        let x: SqliteResult<()> = go();
        let err = x.err().unwrap();
        assert_eq!(err.detail(), None)
    }

    #[test]
    fn no_alloc_errors_stmt() {
        let db = DatabaseConnection::in_memory().unwrap();
        let mut stmt = db.prepare("select 1").unwrap();
        stmt.ignore_detail();
        let oops = stmt.bind_text(3, "abc");
        assert_eq!(oops.err().unwrap().detail(), None)
    }


}

// Local Variables:
// flycheck-rust-crate-root: "lib.rs"
// End:
