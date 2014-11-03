//! Type conversions for binding parameters and getting query results.

use super::{PreparedStatement, ResultRow};
use super::{SqliteError, SqliteResult, SQLITE_MISMATCH};
use super::{SQLITE_NULL};
use time;

/// Values that can be bound to parameters in prepared statements.
pub trait ToSql {
    /// Bind the `ix`th parameter to this value (`self`).
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
///   - *TODO: consider a `types` submodule*
///   - *TODO: many more implementors, including Option<T>*
pub trait FromSql {
    /// Try to extract a `Self` type value from the `col`th colum of a `ResultRow`.
    fn from_sql(row: &mut ResultRow, col: uint) -> SqliteResult<Self>;
}

impl ToSql for i32 {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        s.bind_int(ix, *self)
    }
}

impl FromSql for i32 {
    fn from_sql(row: &mut ResultRow, col: uint) -> SqliteResult<i32> { Ok(row.column_int(col)) }
}

impl ToSql for i64 {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        s.bind_int64(ix, *self)
    }
}

impl FromSql for i64 {
    fn from_sql(row: &mut ResultRow, col: uint) -> SqliteResult<i64> { Ok(row.column_int64(col)) }
}

impl ToSql for f64 {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        s.bind_double(ix, *self)
    }
}

impl FromSql for f64 {
    fn from_sql(row: &mut ResultRow, col: uint) -> SqliteResult<f64> { Ok(row.column_double(col)) }
}

impl<T: ToSql + Clone> ToSql for Option<T> {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        match (*self).clone() {
            Some(x) => x.to_sql(s, ix),
            None => s.bind_null(ix)
        }
    }
}

impl<T: FromSql + Clone> FromSql for Option<T> {
    fn from_sql(row: &mut ResultRow, col: uint) -> SqliteResult<Option<T>> {
        match row.column_type(col) {
            SQLITE_NULL => Ok(None),
            _ => FromSql::from_sql(row, col).map(|x| Some(x))
        }
    }
}

impl ToSql for String {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        s.bind_text(ix, (*self).as_slice())
    }
}


impl FromSql for String {
    fn from_sql(row: &mut ResultRow, col: uint) -> SqliteResult<String> {
        Ok(row.column_text(col).unwrap_or("".to_string()))
    }
}

/// Format of sqlite date strings
///
/// From [Date And Time Functions][lang_datefunc]:
/// > The datetime() function returns "YYYY-MM-DD HH:MM:SS"
/// [lang_datefunc]: http://www.sqlite.org/lang_datefunc.html
pub static SQLITE_TIME_FMT: &'static str = "%F %T";

impl FromSql for time::Tm {
    fn from_sql(row: &mut ResultRow, col: uint) -> SqliteResult<time::Tm> {
        match row.column_text(col) {
            None => Err(SqliteError::new(SQLITE_MISMATCH, "null".to_string(), None)),
            Some(txt) => match time::strptime(txt.as_slice(), SQLITE_TIME_FMT) {
                Ok(tm) => Ok(tm),
                Err(msg) => Err(SqliteError::new(SQLITE_MISMATCH, format!("{}", msg), None))
            }
        }
    }
}


impl ToSql for time::Timespec {
    fn to_sql(&self, s: &mut PreparedStatement, ix: uint) -> SqliteResult<()> {
        match time::at_utc(*self).strftime(SQLITE_TIME_FMT) {
            Ok(text) => s.bind_text(ix, text.as_slice()),
            Err(oops) => Err(SqliteError::new(SQLITE_MISMATCH, format!("{}", oops), None))
        }
    }
}

impl FromSql for time::Timespec {
    /// TODO: propagate error message
    fn from_sql(row: &mut ResultRow, col: uint) -> SqliteResult<time::Timespec> {
        let tmo: SqliteResult<time::Tm> = FromSql::from_sql(row, col);
        tmo.map(|tm| tm.to_timespec())
    }
}

#[cfg(test)]
mod tests {
    use time::Tm;
    use super::super::{DatabaseConnection, SqliteResult};
    use super::super::{ResultRowAccess};

    #[test]
    fn get_tm() {
        fn go() -> SqliteResult<()> {
            let mut conn = try!(DatabaseConnection::in_memory());
            let mut stmt = try!(
                conn.prepare("select datetime('2001-01-01', 'weekday 3', '3 hours')"));
            let mut results = stmt.exec_query();
            match results.step() {
                Some(Ok(ref mut row)) => {
                    assert_eq!(
                        row.get::<uint, Tm>(0u),
                        Tm { tm_sec: 0,
                             tm_min: 0,
                             tm_hour: 3,
                             tm_mday: 3,
                             tm_mon: 0,
                             tm_year: 101,
                             tm_wday: 0,
                             tm_yday: 0,
                             tm_isdst: 0,
                             tm_gmtoff: 0,
                             tm_nsec: 0
                        });
                    Ok(())
                },
                None => panic!("no row"),
                Some(Err(oops)) =>  panic!("error: {}", oops)
            }
        }
        go().unwrap();
    }
}

// Local Variables:
// flycheck-rust-crate-root: "lib.rs"
// End:
