extern crate time;
extern crate sqlite3;

use time::Timespec;


use sqlite3::{DatabaseConnection, Row, Error, Done, SqliteResult};
use sqlite3::{SQLITE_NULL, SQLITE_TEXT};

#[deriving(Show)]
struct Person {
    id: i32,
    name: String,
    time_created: Timespec,
    // TODO: data: Option<Vec<u8>>
}

pub fn main() {
    match io() {
        Ok(ppl) => println!("Found people: {}", ppl),
        Err(oops) => fail!(oops)
    }
}

fn io() -> SqliteResult<Vec<Person>> {
    let mut conn = try!(DatabaseConnection::new());

    try!(conn.exec("CREATE TABLE person (
                 id              SERIAL PRIMARY KEY,
                 name            VARCHAR NOT NULL,
                 time_created    TIMESTAMP NOT NULL
               )"));

    let me = Person {
        id: 0,
        name: "Dan".to_string(),
        time_created: time::get_time(),
    };
    {
        let mut tx = try!(conn.prepare("INSERT INTO person (name, time_created)
                           VALUES ($1, $2)"));
        let changes = try!(tx.update([&me.name, &me.time_created]));
        assert_eq!(changes, 1);
    }

    let mut stmt = try!(conn.prepare("SELECT id, name, time_created FROM person"));
    let mut rows = try!(stmt.query([]));

    let mut ppl = vec!();

    loop {
        match rows.step() {
            Row(ref mut row) => {
                assert_eq!(row.column_type(0), SQLITE_NULL);
                assert_eq!(row.column_type(1), SQLITE_TEXT);
                assert_eq!(row.column_type(2), SQLITE_TEXT);

                ppl.push(Person {
                    id: row.get(0u),
                    name: row.get(1u),
                    time_created: row.get(2u)
                })
            },
            Error(oops) => return Err(oops),
            Done(_) => break
        }
    }
    Ok(ppl)
}
