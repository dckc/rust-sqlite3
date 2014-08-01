// Safe
// inspired by http://www.rust-ci.org/sfackler/rust-postgres/
use std::fmt::Show;

use super::{SqliteRow};
use super::{SqliteResult, SQLITE_MISUSE};

impl<'s, 'r> super::SqliteRow<'s, 'r> {
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
