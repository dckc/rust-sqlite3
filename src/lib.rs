// largely derivative of https://github.com/linuxfood/rustsqlite
// inspired by http://www.rust-ci.org/sfackler/rust-postgres/

#![crate_name = "sqlite3"]
#![crate_type = "lib"]
#![feature(unsafe_destructor)]
#![feature(globs)]

extern crate libc;

use std::fmt::Show;

pub use core::{SqliteConnection, SqliteStatement, SqliteRows, SqliteRow};

// Any code that requires unsafe {} blocks is in mod core.
mod core;

#[allow(non_camel_case_types, dead_code)]
mod ffi;

impl<'s, 'r> core::SqliteRow<'s, 'r> {
    pub fn get<I: RowIndex + Show + Clone, T: FromSql>(&mut self, idx: I) -> T {
        match self.get_opt(idx.clone()) {
            Ok(ok) => ok,
            Err(err) => fail!("retrieving column {}: {}", idx, err)
        }
    }

    pub fn get_opt<I: RowIndex, T: FromSql>(&mut self, idx: I) -> SqliteResult<T> {
        match idx.idx(self) {
            Some(idx) => FromSql::from_sql(self, idx),
            None => Err(SQLITE_MISUSE)
        }
    }

}


trait FromSql {
    // row is provided in case you want to get the sqlite type of that col
    fn from_sql(row: &SqliteRow, col: uint) -> SqliteResult<Self>;
}

impl FromSql for i32 {
    fn from_sql(row: &SqliteRow, col: uint) -> SqliteResult<i32> { Ok(row.column_int(col)) }
}

// inspired by http://www.rust-ci.org/sfackler/rust-postgres/doc/postgres/trait.RowIndex.html
pub trait RowIndex {
    fn idx(&self, row: &mut SqliteRow) -> Option<uint>;
}

impl RowIndex for uint {
    fn idx(&self, _row: &mut SqliteRow) -> Option<uint> { Some(*self) }
}

impl RowIndex for &'static str {
    fn idx(&self, row: &mut SqliteRow) -> Option<uint> {
        let mut ixs = range(0, row.column_count());
        ixs.find(|ix| row.with_column_name(*ix, false, |name| name == *self))
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
