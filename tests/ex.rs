extern crate time;
extern crate sqlite3;

use time::Timespec;


use sqlite3::{DatabaseConnection, Row, Error, Done, SqliteResult};

#[deriving(Show)]
struct Person {
    id: i32,
    name: String,
    time_created: Timespec,
    // TODO: data: Option<Vec<u8>>
}

pub fn main() {
    println!("hello!");
    match io() {
        Ok(_) => (),
        Err(oops) => fail!(oops)
    }
}

fn io() -> SqliteResult<()> {
    let mut conn = try!(DatabaseConnection::new());

    try!(conn.exec("CREATE TABLE person (
                 id              SERIAL PRIMARY KEY,
                 name            VARCHAR NOT NULL,
                 time_created    TIMESTAMP NOT NULL
               )"));
    println!("created");

    let me = Person {
        id: 0,
        name: "Dan".to_string(),
        time_created: time::get_time(),
    };
    {
        let mut tx = try!(conn.prepare("INSERT INTO person (name, time_created)
                           VALUES ($1, $2)"));
        let changes = try!(tx.update([&me.name, &me.time_created]));
        println!("inserted {} {}", changes, me);
    }

    let mut stmt = try!(conn.prepare("SELECT id, name, time_created FROM person"));
    let mut rows = try!(stmt.query([]));
    println!("selecting");
    loop {
        match rows.step() {
            Row(ref mut row) => {
                println!("type of row 0: {}", row.column_type(0));
                println!("type of row 1: {}", row.column_type(1));
                println!("type of row 2: {}", row.column_type(2));
                println!("text of row 2: {}", row.column_text(2));

                let person = Person {
                    id: row.get(0u),
                    name: row.get(1u),
                    time_created: row.get(2u)
                };
                println!("Found person {}", person);
            },
            Error(oops) => return Err(oops),
            Done(_) => break
        }
    }
    Ok(())
}
