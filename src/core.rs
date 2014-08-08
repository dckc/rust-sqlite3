//! A minimal safe interface to sqlite3's basic API.
//!
//! The basic sqlite3 API is discussed in the [sqlite intro][intro].
//! To go beyond that, use the (unsafe) `ffi` module directly.
//!
//! [intro]: http://www.sqlite.org/cintro.html

use libc::{c_int};
use std::num::from_i32;
use std::ptr;
use std::mem;
use std::c_str;

use super::{SQLITE_OK, SqliteError, StepOutcome, SqliteResult};
use super::{ParameterValue, Text, Blob, Integer, Integer64, Float64, Null};
use super::{Done, Row, Error};

use ffi;

/// A connection to a sqlite3 database.
pub struct DatabaseConnection {
    // not pub so that nothing outside this module
    // interferes with the lifetime
    db: *mut ffi::sqlite3
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
        // sqlite3_close_v2 was not introduced until 2012-09-03 (3.7.14)
        // but we want to build on, e.g. travis, i.e. Ubuntu 12.04.
        // let ok = unsafe { ffi::sqlite3_close_v2(self.db) };
        let ok = unsafe { ffi::sqlite3_close(self.db) };
        assert_eq!(ok, SQLITE_OK as c_int);
    }
}


/// Authorization to connect to database.
///
/// *TODO: mark this unsafe?*
pub type Access = proc(*mut *mut ffi::sqlite3) -> c_int;

impl DatabaseConnection {
    /// Create connection to an in-memory database.
    ///
    ///  - TODO: use support _v2 interface with flags
    ///  - TODO: integrate sqlite3_errmsg()
    pub fn new() -> SqliteResult<DatabaseConnection> {
        fn in_memory(db: *mut *mut ffi::sqlite3) -> c_int {
            let result = ":memory:".with_c_str({
                |memory| unsafe { ffi::sqlite3_open(memory, db) }
            });
            result
        }
        DatabaseConnection::connect(in_memory)
    }

    /// Given explicit access to a database, attempt to connect to it.
    ///
    /// *TODO: mark this unsafe?*
    #[allow(visible_private_types)]
    pub fn connect(open: Access) -> SqliteResult<DatabaseConnection> {
        let mut db = ptr::mut_null();
        let result = open(&mut db);
        match decode_result(result, "sqlite3_open") {
            Ok(()) => Ok(DatabaseConnection { db: db }),
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

    /// Prepare/compile an SQL statement.
    pub fn prepare<'db>(&'db mut self, sql: &str) -> SqliteResult<PreparedStatement<'db>> {
        match self.prepare_with_offset(sql) {
            Ok((cur, _)) => Ok(cur),
            Err(e) => Err(e)
        }
    }
                
    pub fn prepare_with_offset<'db>(&'db mut self, sql: &str)
                                    -> SqliteResult<(PreparedStatement<'db>, uint)> {
        let mut stmt = ptr::mut_null();
        let mut tail = ptr::null();
        let z_sql = sql.as_ptr() as *const ::libc::c_char;
        let n_byte = sql.len() as c_int;
        let r = unsafe { ffi::sqlite3_prepare_v2(self.db, z_sql, n_byte, &mut stmt, &mut tail) };
        match decode_result(r, "sqlite3_prepare_v2") {
            Ok(()) => {
                let offset = tail as uint - z_sql as uint;
                Ok((PreparedStatement { stmt: stmt, conn: self }, offset))
            },
            Err(code) => Err(code)
        }
    }

    /// One-Step Query Execution Interface
    ///
    /// cf [sqlite3_exec][exec]
    /// [exec]: http://www.sqlite.org/c3ref/exec.html
    ///
    ///  - TODO: callback support?
    ///  - TODO: errmsg support
    pub fn exec(&mut self, sql: &str) -> SqliteResult<()> {
        let result = sql.with_c_str(
            |c_sql| unsafe { ffi::sqlite3_exec(self.db, c_sql, None,
                                               ptr::mut_null(), ptr::mut_null()) });
        decode_result(result, "sqlite3_exec")
    }

    /// Expose the underlying `sqlite3` struct pointer for use
    /// with the `ffi` module.
    pub unsafe fn expose(&mut self) -> *mut ffi::sqlite3 {
        self.db
    }
}


/// A prepared statement.
pub struct PreparedStatement<'db> {
    conn: &'db mut DatabaseConnection,
    stmt: *mut ffi::sqlite3_stmt
}

#[unsafe_destructor]
impl<'db> Drop for PreparedStatement<'db> {
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


impl<'db> PreparedStatement<'db> {
    /// Execute a statement after binding any parameters.
    ///
    /// The `want_changes` argument determines whether the [number
    /// of rows modified][changes] is reported when the statement is done.
    /// (See `ResultSet::step()`.)
    ///
    /// *TODO: support binding by name as well as by position?*
    ///
    /// [changes]: http://www.sqlite.org/c3ref/changes.html
    pub fn execute(&'db mut self, want_changes: bool,
                   // TODO: figure out why &[ParameterValue] doesn't work.
                   values: Vec<ParameterValue>)
                   -> SqliteResult<ResultSet<'db>> {
        {
            let r = unsafe { ffi::sqlite3_reset(self.stmt) };
            try!(decode_result(r, "sqlite3_reset"));
        }

        {
            let r = unsafe { ffi::sqlite3_clear_bindings(self.stmt) };
            assert_eq!(r, 0);
        }

        // SQL parameter index (starting from 1).
        for (i, v) in values.iter().enumerate() {
            try!(self.bind(i + 1, v))
        }

        Ok(ResultSet { statement: self, want_changes: want_changes })
    }

    /// Bind a value to a statement parameter.
    ///
    /// **Note:** "The leftmost SQL parameter has an index of 1."[1]
    ///
    /// *TODO: support binding without copying strings, blobs*
    ///
    /// [1]: http://www.sqlite.org/c3ref/bind_blob.html
    pub fn bind(&mut self, i: uint, value: &ParameterValue) -> SqliteResult<()> {
        //debug!("`Cursor.bind_param(stmt={:?}, i={:?}, value={})`", self.stmt, i, value);

        let ix = i as c_int;
        // SQLITE_TRANSIENT => SQLite makes a copy
        let transient = unsafe { mem::transmute(-1i) };

        let r = match *value {
            Null => { unsafe { ffi::sqlite3_bind_null(self.stmt, ix ) } },
            Integer(ref v) => { unsafe { ffi::sqlite3_bind_int(self.stmt, ix, *v as c_int) } },
            Integer64(ref v) => { unsafe { ffi::sqlite3_bind_int64(self.stmt, ix, *v) } },
            Float64(ref v) => { unsafe { ffi::sqlite3_bind_double(self.stmt, ix, *v) } },

            Text(ref v) => {
                let len = v.len() as c_int;
                //debug!("  `Text`: v={:?}, l={:?}", v, l);

                (*v).with_c_str( |_v| {
                    unsafe { ffi::sqlite3_bind_text(self.stmt, ix, _v, len, transient) }
                })
            },

            Blob(ref v) => {
                let val = unsafe { mem::transmute(v.as_ptr()) };
                let len = v.len() as c_int;
                //debug!("`Blob`: v={:?}, l={:?}", v, l);

                unsafe { ffi::sqlite3_bind_blob(self.stmt, ix, val, len, transient) }
            }
        };

        decode_result(r, "sqlite3_bind_...")
    }

    /// Expose the underlying `sqlite3_stmt` struct pointer for use
    /// with the `ffi` module.
    pub unsafe fn expose(&mut self) -> *mut ffi::sqlite3_stmt {
        self.stmt
    }
}


/// Results of executing a `prepare()`d statement.
pub struct ResultSet<'s> {
    statement: &'s mut PreparedStatement<'s>,
    want_changes: bool
}

#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
enum Step {
    SQLITE_ROW       = 100,
    SQLITE_DONE      = 101,
}


impl<'s> ResultSet<'s> {
    /// Iterate over rows resulting from execution of a prepared statement.
    ///
    /// An sqlite "row" only lasts until the next call to `ffi::sqlite3_step()`,
    /// so we need a lifetime constraint. The unfortunate result is that
    ///  `ResultSet` cannot implement the `Iterator` trait.
    pub fn step<'r>(&'r mut self) -> StepOutcome<'s, 'r> {
        let result = unsafe { ffi::sqlite3_step(self.statement.stmt) };
        match from_i32::<Step>(result) {
            Some(SQLITE_ROW) => {
                Row(ResultRow{ rows: self })
            },
            Some(SQLITE_DONE) => Done({
                match self.want_changes {
                    true => {
                        let db = self.statement.conn.db;
                        let count = unsafe { ffi::sqlite3_changes(db) };
                        Some(count as uint)
                    }
                    false => None
                }
            }),
            None => {
                let err = from_i32::<SqliteError>(result);
                Error(err.unwrap())
            }
        }
    }
}


/// Access to columns of a row.
pub struct ResultRow<'s, 'r> {
    rows: &'r mut ResultSet<'s>
}

impl<'s, 'r> ResultRow<'s, 'r> {

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

    pub fn column_text(&self, col: uint) -> Option<String> {
        let stmt = self.rows.statement.stmt;
        let i_col = col as c_int;
        match unsafe {
            let s = ffi::sqlite3_column_text(stmt, i_col);
            if s == ptr::null() { None }
            else { Some(c_str::CString::new(mem::transmute(s), true)) }
        } {
            Some(c_str) => match c_str.as_str() {
                Some(str) => Some(str.to_string()),
                None => None
            },
            None => None
        }
    }


}


#[inline]
pub fn decode_result(result: c_int, context: &str) -> SqliteResult<()> {
    if result == SQLITE_OK as c_int {
        Ok(())
    } else {
        match from_i32::<SqliteError>(result) {
            Some(code) => Err(code),
            None => fail!("{} returned unexpected {:d}", context, result)
        }
    }
}

// Local Variables:
// flycheck-rust-crate-root: "lib.rs"
// End:
