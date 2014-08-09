extern crate sqlite3;

use sqlite3::{DatabaseConnection, SqliteResult, Row, Done, Error};

fn convenience_exec() -> SqliteResult<DatabaseConnection> {
    let mut conn = try!(DatabaseConnection::new());

    try!(conn.exec("
       create table items (
                   id integer,
                   description varchar(40),
                   price integer
                   )"));

    Ok(conn)
 }

fn typical_usage(conn: &mut DatabaseConnection) -> SqliteResult<String> {
    {
        let mut stmt = try!(conn.prepare(
            "insert into items (id, description, price)
           values (1, 'stuff', 10)"));
        let mut results = stmt.execute(true);
        let changes = match results.step() {
            Done(Some(qty)) => qty,
            Done(None) => fail!("cannot happen; we gave true to execute()"),
            Row(_) => fail!("row from insert?!"),
            Error(oops) => fail!(oops)
        };
        assert_eq!(changes, 1);
    }
    {
        let mut stmt = try!(conn.prepare(
            "select * from items"));
        let mut results = stmt.execute(false);
        match results.step() {
            Row(ref mut row1) => {
                let id = row1.column_int(0);
                let desc_opt = row1.column_text(1).expect("no desc?!");
                let price = row1.column_int(2);

                assert_eq!(id, 1);
                assert_eq!(desc_opt, "stuff".to_string());
                assert_eq!(price, 10);

                Ok(format!("row: {}, {}, {}", id, desc_opt, price))
            },
            Done(_) => fail!("where did our row go?"),
            Error(oops) => fail!(oops)
        }
    }
}

pub fn main() {
    match convenience_exec() {
        Ok(ref mut db) => {
            match typical_usage(db) {
                Ok(txt) => println!("item: {}", txt),
                Err(oops) => {
                    fail!("error: {} msg: {}", oops,
                          db.errmsg())
                }
            }
        },
        Err(oops) => fail!(oops)
    }
}
