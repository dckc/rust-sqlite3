extern crate sqlite3;

use std::os;

use sqlite3::{DatabaseConnection, DatabaseUpdate,
              Query, ResultRowAccess,
              SqliteResult, SqliteError};
use sqlite3::access;

#[deriving(Show)]
struct Person {
    id: i32,
    name: String,
}

pub fn main() {
    let db = os::args()[1].clone(); // TODO: no I/O in main
    let access = access::filename_access(db);

    match io(access) {
        Ok(x) => println!("Ok: {}", x),
        Err(oops) => fail!("oops!: {}", oops)
    }
}

fn io(access: sqlite3::Access) -> Result<Vec<Person>, (SqliteError, String)> {
    match DatabaseConnection::new(access) {
        Ok(ref mut conn) => match io2(conn) {
            Ok(ppl) => Ok(ppl),
            Err(oops) => Err((oops, conn.errmsg()))
        },
        Err(oops) => Err(oops)
    }
}

fn io2(conn: &mut DatabaseConnection) -> SqliteResult<Vec<Person>> {
    try!(conn.exec("CREATE TABLE person (
                 id              SERIAL PRIMARY KEY,
                 name            VARCHAR NOT NULL
               )"));

    {
        let mut tx = try!(conn.prepare("INSERT INTO person (id, name)
                           VALUES (0, 'Dan')"));
        let changes = try!(conn.update(&mut tx, []));
        assert_eq!(changes, 1);
    }

    let mut stmt = try!(conn.prepare("SELECT id, name FROM person"));

    let mut ppl = vec!();
    try!(stmt.query(
        [], |row| {
            ppl.push(Person {
                id: row.get(0u),
                name: row.get(1u)
            });
            Ok(())
        }));
    Ok(ppl)
}
