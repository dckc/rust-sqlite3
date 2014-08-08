//! Type conversions for binding parameters and getting query results.

use super::{ResultRow, SqliteResult, SQLITE_MISMATCH};
use super::{ParameterValue,
            Null,
            Integer,
            Integer64,
            Float64,
            Text};
use time;

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

pub trait ToSql {
    fn to_sql(&self) -> ParameterValue;
}

impl FromSql for i32 {
    fn from_sql(row: &ResultRow, col: uint) -> SqliteResult<i32> { Ok(row.column_int(col)) }
}

impl FromSql for int {
    // TODO: get_int should take a uint, not an int, right?
    fn from_sql(row: &ResultRow, col: uint) -> SqliteResult<int> { Ok(row.column_int(col) as int) }
}

impl ToSql for int {
    fn to_sql(&self) -> ParameterValue { Integer(*self) }
}

impl ToSql for i64 {
    fn to_sql(&self) -> ParameterValue { Integer64(*self) }
}

impl ToSql for f64 {
    fn to_sql(&self) -> ParameterValue { Float64(*self) }
}

impl<T: ToSql + Clone> ToSql for Option<T> {
    fn to_sql(&self) -> ParameterValue {
        match (*self).clone() {
            Some(x) => x.to_sql(),
            None => Null
        }
    }
}

impl FromSql for String {
    fn from_sql(row: &ResultRow, col: uint) -> SqliteResult<String> {
        Ok(row.column_text(col).to_string())
    }
}


impl ToSql for String {
    // TODO: eliminate copy?
    fn to_sql(&self) -> ParameterValue { Text(self.clone()) }
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
    fn to_sql(&self) -> ParameterValue {
        Text(time::at_utc(*self).rfc3339())
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
