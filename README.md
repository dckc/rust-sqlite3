# Rust-Sqlite3

Rustic bindings for sqlite3.

Copyright (c) 2014 [Dan Connolly][dckc]

Share and enjoy. LICENSE: MIT.

[dckc]: http://www.madmode.com/


## Status

Basic API design issues worked out, but only a handful of features supported.

 - TODO: docs at rust-ci
 - TODO: build status: travis-ci
 - TODO: the other 95% of the relevant features

## Motivation and Acknowledgements

I was looking into [sandstorm][], a personal cloud platform with an
architecture based on the wonderful [capability security][capsec]
paradigm, and I found a rust application, [acronymy][], that uses the
native API rather than the traditional POSIX environment.

[sandstorm]: https://sandstorm.io/
[capsec]: http://www.erights.org/elib/capability/ode/ode-capabilities.html
[acronymy]: https://github.com/dwrensha/acronymy

I started poring over the code and followed the dependency link to
linuxfood's [rustsqlite][]. I started working on a [memory safety
issue][92] etc. but soon found a number of large-scale API design
issues that I wasn't sure how to approach with the upstream
developers. I was also inspired by `FromSql`, `ToSql` and such
from sfackler's [rust-postgres] API.

So I started from scratch, using [bindgen][], `Result` (sum types) etc.

[rustsqlite]: https://github.com/linuxfood/rustsqlite
[92]: https://github.com/linuxfood/rustsqlite/issues/92
[rust-postgres]: https://github.com/sfackler/rust-postgres
[bindgen]: https://github.com/crabtw/rust-bindgen
