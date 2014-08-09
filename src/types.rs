//! Type conversions for binding parameters and getting query results.

use super::{PreparedStatement, ResultRow};
use super::{SqliteResult, SQLITE_MISMATCH};
use time;

pub trait ToSql {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()>;
}

/// A trait for result values from a query.
///
/// cf [sqlite3 result values][column].
///
/// *inspired by sfackler's FromSql (and some haskell bindings?)*
///
/// [column]: http://www.sqlite.org/c3ref/column_blob.html
///
///   - TODO: consider a `types` submodule
///   - TODO: many more implementors, including Option<T>
pub trait FromSql {
    fn from_sql(row: &ResultRow, col: uint) -> SqliteResult<Self>;
}

impl ToSql for i32 {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        s.bind_int(ix, *self)
    }
}

impl FromSql for i32 {
    fn from_sql(row: &ResultRow, col: uint) -> SqliteResult<i32> { Ok(row.column_int(col)) }
}

impl ToSql for i64 {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        s.bind_int64(ix, *self)
    }
}

impl FromSql for i64 {
    fn from_sql(row: &ResultRow, col: uint) -> SqliteResult<i64> { Ok(row.column_int64(col)) }
}

impl ToSql for f64 {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        s.bind_double(ix, *self)
    }
}

impl FromSql for f64 {
    fn from_sql(row: &ResultRow, col: uint) -> SqliteResult<f64> { Ok(row.column_double(col)) }
}

impl<T: ToSql + Clone> ToSql for Option<T> {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        match (*self).clone() {
            Some(x) => x.to_sql(s, ix),
            None => s.bind_null(ix)
        }
    }
}

impl ToSql for String {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        s.bind_text(ix, (*self).as_slice())
    }
}


impl FromSql for String {
    fn from_sql(row: &ResultRow, col: uint) -> SqliteResult<String> {
        Ok(row.column_text(col).to_string())
    }
}


impl FromSql for time::Tm {
    /// TODO: propagate error message
    fn from_sql(row: &ResultRow, col: uint) -> SqliteResult<time::Tm> {
        let isofmt = "%F";  // YYYY-MM-DD
        match row.column_text(col) {
            None => Err(SQLITE_MISMATCH),
            Some(txt) => match time::strptime(txt.as_slice(), isofmt) {
                Ok(tm) => Ok(tm),
                Err(msg) => Err(SQLITE_MISMATCH)
            }
        }
    }
}


impl ToSql for time::Timespec {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        s.bind_text(ix, time::at_utc(*self).rfc3339().as_slice())
    }
}

impl FromSql for time::Timespec {
    /// TODO: propagate error message
    fn from_sql(row: &ResultRow, col: uint) -> SqliteResult<time::Timespec> {
        let tmo: SqliteResult<time::Tm> = FromSql::from_sql(row, col);
        tmo.map(|tm| tm.to_timespec())
    }
}

// Local Variables:
// flycheck-rust-crate-root: "lib.rs"
// End:
