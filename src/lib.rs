//! `rust-sqlite3` is a rustic binding to the [sqlite3 API][].
//!
//! [sqlite3 API]: http://www.sqlite.org/c3ref/intro.html
//!
//! ```rust
//! extern crate time;
//! extern crate sqlite3;
//!
//! use time::Timespec;
//!
//!
//! use sqlite3::{DatabaseConnection, SqliteResult, SqliteError};
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
//!         Err(oops) => fail!(oops)
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
//!         let changes = try!(conn.update(&mut tx, [&me.name, &me.time_created]));
//!         assert_eq!(changes, 1);
//!     }
//!
//!     let mut stmt = try!(conn.prepare("SELECT id, name, time_created FROM person"));
//!
//!     let mut ppl = vec!();
//!     try!(stmt.query(
//!         [], |row| {
//!             ppl.push(Person {
//!                 id: row.get(0u),
//!                 name: row.get(1u),
//!                 time_created: row.get(2u)
//!             });
//!             Ok(())
//!         }));
//!     Ok(ppl)
//! }
//! ```

#![crate_name = "sqlite3"]
#![crate_type = "lib"]
#![feature(unsafe_destructor,unboxed_closures)]

extern crate libc;
extern crate time;

use std::fmt::Show;

pub use core::Access;
pub use core::{DatabaseConnection, PreparedStatement, ResultSet, ResultRow};
pub use types::{FromSql, ToSql};

pub mod core;
pub mod types;

/// bindgen-bindings to libsqlite3
#[allow(non_camel_case_types, dead_code)]
pub mod ffi;

pub mod access;


impl core::DatabaseConnection {
    /// Execute a statement after binding any parameters.
    ///
    /// When the statement is done, The [number of rows
    /// modified][changes] is reported.
    ///
    /// [changes]: http://www.sqlite.org/c3ref/changes.html
    pub fn update<'db, 's>(&'db mut self,
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

impl<'s> core::PreparedStatement<'s> {
    /// Process rows from a query after binding parameters.
    pub fn query(&'s mut self,
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


impl<'s, 'r> core::ResultRow<'s, 'r> {
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

/// A trait implemented by types that can index into columns of a row.
///
/// *inspired by sfackler's [RowIndex][]*
/// [RowIndex]: http://www.rust-ci.org/sfackler/rust-postgres/doc/postgres/trait.RowIndex.html
pub trait RowIndex {
    fn idx(&self, row: &mut ResultRow) -> Option<uint>;
}

impl RowIndex for uint {
    fn idx(&self, _row: &mut ResultRow) -> Option<uint> { Some(*self) }
}

impl RowIndex for &'static str {
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
                _ => fail!()
            }

            match rows.step() {
                Some(Ok(ref mut row)) => {
                    assert_eq!(row.get::<uint, i32>(0), 2);
                    //TODO let addr = q.get_text(2);
                    // assert_eq!(addr.as_slice(), "345 e Walnut");
                },
                _ => fail!()
            }
            Ok(())
        }
        match go() {
            Ok(_) => (),
            Err(e) => fail!("oops! {}", e)
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
