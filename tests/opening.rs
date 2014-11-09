extern crate sqlite3;

use std::default::Default;
use std::error::FromError;
use std::io::{IoResult, IoError, InvalidInput};
use std::os;

use sqlite3::{
    Access,
    DatabaseConnection,
    DatabaseUpdate,
    Query,
    ResultRowAccess,
    SqliteResult,
};
use sqlite3::access;
use sqlite3::access::flags::OPEN_READONLY;

pub fn main() {
    let args = os::args();
    let cli_access = {
        // We want to use FromError below, so use IoError rather than just a string.
        let usage = IoError{
            kind: InvalidInput,
            desc: "args: [-r] filename",
            detail: None };

        let ok = |flags, dbfile| Ok(access::ByFilename { flags: flags, filename: dbfile });

        let arg = |n| {
            if args.len() > n { Some(args[n].as_slice()) }
            else { None }
        };

        match (arg(1), arg(2)) {
            (Some("-r"), Some(dbfile))
                => ok(OPEN_READONLY, dbfile),
            (Some(dbfile), None)
                => ok(Default::default(), dbfile),
            (_, _)
                => Err(usage)
        }
    };

    fn use_access<A: Access>(access: A) -> IoResult<Vec<Person>> {
        let mut conn = try!(DatabaseConnection::new(access));
        make_people(&mut conn)
            .map_err(|e| e.with_detail(conn.errmsg()))
            .map_err(|e| FromError::from_error(e))
    }

    match cli_access.and_then(use_access) {
        Ok(x) => println!("Ok: {}", x),
        Err(oops) => {
            std::os::set_exit_status(1);
            // writeln!() macro acts like a statement; hence the extra ()s
            (writeln!(std::io::stderr(), "oops!: {}", oops)).unwrap()
        }
    };
}


#[deriving(Show)]
struct Person {
    id: i32,
    name: String,
}

fn make_people(conn: &mut DatabaseConnection) -> SqliteResult<Vec<Person>> {
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

// Local Variables:
// flycheck-rust-library-path: ("../target")
// End:
