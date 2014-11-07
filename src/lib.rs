//! `rust-sqlite3` is a rustic binding to the [sqlite3 API][].
//!
//! [sqlite3 API]: http://www.sqlite.org/c3ref/intro.html
//!
//! Three layers of API are provided:
//!
//!  - `mod ffi` provides exhaustive, though unsafe, [bindgen] bindings for `libsqlite.h`.
//!  - `mod core` provides a minimal safe interface to the basic sqlite3 API.
//!  - `mod types` provides `ToSql`/`FromSql` traits, and the library provides
//!     convenient `query()` and `update()` APIs.
//!
//! [bindgen]: https://github.com/crabtw/rust-bindgen
//!
//! The following example demonstrates opening a database, executing
//! DDL, and using the high-level `query()` and `update()` API. Note the
//! use of `Result` and `try!()` for error handling.
//!
//! ```rust
//! extern crate time;
//! extern crate sqlite3;
//!
//! use time::Timespec;
//!
//!
//! use sqlite3::{DatabaseConnection, DatabaseUpdate,
//!               Query, ResultRowAccess,
//!               SqliteResult, SqliteError};
//!
//! #[deriving(Show)]
//! struct Person {
//!     id: i32,
//!     name: String,
//!     time_created: Timespec,
//!     // TODO: data: Option<Vec<u8>>
//! }
//!
//! pub fn main() {
//!     match io() {
//!         Ok(ppl) => println!("Found people: {}", ppl),
//!         Err(oops) => panic!(oops)
//!     }
//! }
//!
//! fn io() -> Result<Vec<Person>, (SqliteError, String)> {
//!     let mut conn = try!(DatabaseConnection::in_memory());
//!     with_conn(&mut conn).map_err(|code| (code, conn.errmsg()))
//! }
//!
//! fn with_conn(conn: &mut DatabaseConnection) -> SqliteResult<Vec<Person>> {
//!     try!(conn.exec("CREATE TABLE person (
//!                  id              SERIAL PRIMARY KEY,
//!                  name            VARCHAR NOT NULL,
//!                  time_created    TIMESTAMP NOT NULL
//!                )"));
//!
//!     let me = Person {
//!         id: 0,
//!         name: "Dan".to_string(),
//!         time_created: time::get_time(),
//!     };
//!     {
//!         let mut tx = try!(conn.prepare("INSERT INTO person (name, time_created)
//!                            VALUES ($1, $2)"));
//!         let changes = try!(conn.update(&mut tx, &[&me.name, &me.time_created]));
//!         assert_eq!(changes, 1);
//!     }
//!
//!     let mut stmt = try!(conn.prepare("SELECT id, name, time_created FROM person"));
//!
//!     let mut ppl = vec!();
//!     try!(stmt.query(
//!         [], |row| {
//!             ppl.push(Person {
//!                 id: row.get("id"),
//!                 name: row.get("name"),
//!                 time_created: row.get(2u)
//!             });
//!             Ok(())
//!         }));
//!     Ok(ppl)
//! }
//! ```

#![crate_name = "sqlite3"]
#![crate_type = "lib"]
#![feature(unsafe_destructor)]
#![warn(missing_docs)]

extern crate libc;
extern crate time;

use std::fmt::Show;

pub use core::Access;
pub use core::{DatabaseConnection, PreparedStatement, ResultSet, ResultRow};
pub use types::{FromSql, ToSql};
pub use consts::{OpenFlags};

pub mod core;
pub mod types;

/// bindgen-bindings to libsqlite3
#[allow(non_camel_case_types, non_snake_case)]
#[allow(dead_code)]
#[allow(missing_docs)]
pub mod ffi;

#[allow(missing_doc)]
pub mod consts;

pub mod access;

/// Mix in `update()` convenience function.
pub trait DatabaseUpdate {
    /// Execute a statement after binding any parameters.
    fn update<'db, 's>(&'db mut self,
                       stmt: &'s mut PreparedStatement<'s>,
                       values: &[&ToSql]) -> SqliteResult<uint>;
}


impl DatabaseUpdate for core::DatabaseConnection {
    /// Execute a statement after binding any parameters.
    ///
    /// When the statement is done, The [number of rows
    /// modified][changes] is reported.
    ///
    /// Fail with `Err(SQLITE_MISUSE)` in case the statement results
    /// in any any rows (e.g. a `SELECT` rather than `INSERT` or
    /// `UPDATE`).
    ///
    /// [changes]: http://www.sqlite.org/c3ref/changes.html
    fn update<'db, 's>(&'db mut self,
                       stmt: &'s mut PreparedStatement<'s>,
                       values: &[&ToSql]) -> SqliteResult<uint> {
        let check = {
            try!(bind_values(stmt, values));
            let mut results = stmt.execute();
            match results.step() {
                None => Ok(()),
                Some(Ok(_row)) => Err(SQLITE_MISUSE),
                Some(Err(e)) => Err(e)
            }
        };
        check.map(|_ok| self.changes())
    }
}


/// Mix in `query()` convenience function.
pub trait Query<'s> {
    /// Process rows from a query after binding parameters.
    fn query(&'s mut self,
             values: &[&ToSql],
             each_row: |&mut ResultRow|: 's -> SqliteResult<()>
             ) -> SqliteResult<()>;
}

impl<'s> Query<'s> for core::PreparedStatement<'s> {
    /// Process rows from a query after binding parameters.
    ///
    /// For call `each_row(row)` for each resulting step,
    /// exiting on `Err`.
    fn query(&'s mut self,
             values: &[&ToSql],
             each_row: |&mut ResultRow|: 's -> SqliteResult<()>
             ) -> SqliteResult<()> {
        try!(bind_values(self, values));
        let mut results = self.execute();
        loop {
            match results.step() {
                None => break,
                Some(Ok(ref mut row)) => try!(each_row(row)),
                Some(Err(e)) => return Err(e)
            }
        }
        Ok(())
    }
}

fn bind_values<'db>(s: &'db mut PreparedStatement, values: &[&ToSql]) -> SqliteResult<()> {
    for (ix, v) in values.iter().enumerate() {
        try!(v.to_sql(s, ix + 1));
    }
    Ok(())
}


/// Access result columns of a row by name or numeric index.
pub trait ResultRowAccess {
    /// Get `T` type result value from `idx`th column of a row.
    ///
    /// # Failure
    ///
    /// Fails if there is no such column or value.
    fn get<I: RowIndex + Show + Clone, T: FromSql>(&mut self, idx: I) -> T;

    /// Try to get `T` type result value from `idx`th column of a row.
    fn get_opt<I: RowIndex, T: FromSql>(&mut self, idx: I) -> SqliteResult<T>;
}

impl<'s, 'r> ResultRowAccess for core::ResultRow<'s, 'r> {
    fn get<I: RowIndex + Show + Clone, T: FromSql>(&mut self, idx: I) -> T {
        match self.get_opt(idx.clone()) {
            Ok(ok) => ok,
            Err(err) => panic!("retrieving column {}: {}", idx, err)
        }
    }

    fn get_opt<I: RowIndex, T: FromSql>(&mut self, idx: I) -> SqliteResult<T> {
        match idx.idx(self) {
            Some(idx) => FromSql::from_sql(self, idx),
            None => Err(SQLITE_MISUSE)
        }
    }

}

/// A trait implemented by types that can index into columns of a row.
///
/// *inspired by sfackler's [RowIndex][]*
/// [RowIndex]: http://www.rust-ci.org/sfackler/rust-postgres/doc/postgres/trait.RowIndex.html
pub trait RowIndex {
    /// Try to convert `self` to an index into a row.
    fn idx(&self, row: &mut ResultRow) -> Option<uint>;
}

impl RowIndex for uint {
    /// Index into a row directly by uint.
    fn idx(&self, _row: &mut ResultRow) -> Option<uint> { Some(*self) }
}

impl RowIndex for &'static str {
    /// Index into a row by column name.
    ///
    /// *TODO: figure out how to use lifetime of row rather than
    /// `static`.*
    fn idx(&self, row: &mut ResultRow) -> Option<uint> {
        let mut ixs = range(0, row.column_count());
        ixs.find(|ix| row.with_column_name(*ix, false, |name| name == *self))
    }
}


/// The type used for returning and propagating sqlite3 errors.
#[must_use]
pub type SqliteResult<T> = Result<T, SqliteError>;

/// Result codes for errors.
///
/// cf. [sqlite3 result codes][codes].
///
/// Note `SQLITE_OK` is not included; we use `Ok(...)` instead.
///
/// Likewise, in place of `SQLITE_ROW` and `SQLITE_DONE`, we return
/// `Some(...)` or `None` from `ResultSet::next()`.
///
/// [codes]: http://www.sqlite.org/c3ref/c_abort.html
#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
#[allow(missing_docs)]
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


/// Fundamental Datatypes
#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
#[allow(missing_docs)]
pub enum ColumnType {
    SQLITE_INTEGER = 1,
    SQLITE_FLOAT   = 2,
    SQLITE_TEXT    = 3,
    SQLITE_BLOB    = 4,
    SQLITE_NULL    = 5
}


#[cfg(test)]
mod bind_tests {
    use super::{DatabaseConnection, ResultSet};
    use super::{ResultRowAccess};
    use super::{SqliteResult};

    #[test]
    fn bind_fun() {
        fn go() -> SqliteResult<()> {
            let mut database = try!(DatabaseConnection::in_memory()
                                    .map_err(|(code, _msg)| code));

            try!(database.exec(
                "BEGIN;
                CREATE TABLE test (id int, name text, address text);
                INSERT INTO test (id, name, address) VALUES (1, 'John Doe', '123 w Pine');
                COMMIT;"));

            {
                let mut tx = try!(database.prepare(
                    "INSERT INTO test (id, name, address) VALUES (?, ?, ?)"));
                try!(tx.bind_int(1, 2));
                try!(tx.bind_text(2, "Jane Doe"));
                try!(tx.bind_text(3, "345 e Walnut"));
                let mut results = tx.execute();
                assert!(results.step().is_none());
            }
            assert_eq!(database.changes(), 1);

            let mut q = try!(database.prepare("select * from test order by id"));
            let mut rows = q.execute();
            match rows.step() {
                Some(Ok(ref mut row)) => {
                    assert_eq!(row.get::<uint, i32>(0), 1);
                    // TODO let name = q.get_text(1);
                    // assert_eq!(name.as_slice(), "John Doe");
                },
                _ => panic!()
            }

            match rows.step() {
                Some(Ok(ref mut row)) => {
                    assert_eq!(row.get::<uint, i32>(0), 2);
                    //TODO let addr = q.get_text(2);
                    // assert_eq!(addr.as_slice(), "345 e Walnut");
                },
                _ => panic!()
            }
            Ok(())
        }
        match go() {
            Ok(_) => (),
            Err(e) => panic!("oops! {}", e)
        }
    }

    fn with_query<T>(sql: &str, f: |rows: &mut ResultSet| -> T) -> SqliteResult<T> {
        let mut db = try!(DatabaseConnection::in_memory()
                          .map_err(|(code, _msg)| code));
        let mut s = try!(db.prepare(sql));
        let mut rows = s.execute();
        Ok(f(&mut rows))
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
                    match rows.step() {
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
