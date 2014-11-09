extern crate sqlite3;

use sqlite3::{
    DatabaseConnection,
    SqliteResult,
};

fn convenience_exec() -> SqliteResult<DatabaseConnection> {
    let mut conn = try!(DatabaseConnection::in_memory());

    try!(conn.exec("
       create table items (
                   id integer,
                   description varchar(40),
                   price integer
                   )")
         .map_err(|err| err.with_detail(conn.errmsg())));

    Ok(conn)
}

fn typical_usage(conn: &mut DatabaseConnection) -> SqliteResult<String> {
    {
        let mut stmt = try!(conn.prepare(
            "insert into items (id, description, price)
           values (1, 'stuff', 10)"));
        let mut results = stmt.execute();
        match results.step() {
            None => (),
            Some(Ok(_)) => panic!("row from insert?!"),
            Some(Err(oops)) => panic!(oops)
        };
    }
    assert_eq!(conn.changes(), 1);
    assert_eq!(conn.last_insert_rowid(), 1);
    {
        let mut stmt = try!(conn.prepare(
            "select * from items"));
        let mut results = stmt.execute();
        match results.step() {
            Some(Ok(ref mut row1)) => {
                let id = row1.column_int(0);
                let desc_opt = row1.column_text(1).expect("no desc?!");
                let price = row1.column_int(2);

                assert_eq!(id, 1);
                assert_eq!(desc_opt, "stuff".to_string());
                assert_eq!(price, 10);

                Ok(format!("row: {}, {}, {}", id, desc_opt, price))
            },
            Some(Err(oops)) => panic!(oops),
            None => panic!("where did our row go?")
        }
    }
}

pub fn main() {
    match convenience_exec() {
        Ok(ref mut db) => {
            match typical_usage(db) {
                Ok(txt) => println!("item: {}", txt),
                Err(oops) => {
                    panic!("error: {} msg: {}", oops,
                          db.errmsg())
                }
            }
        },
        Err(oops) => panic!(oops)
    }
}
