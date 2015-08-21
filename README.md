# hash-rs
Benchmarks of various hashers: http://cglab.ca/~abeinges/blah/hash-rs/

To build the results, run `cargo run` (this will in turn run Cargo bench in the background).
This will produce some csv's that index.html will consume.

Currently only Sip, Fnv, Farm, and XX are supported. Other hasher crates were in an inappropriate state.
Patches to change this welcome!

This does not necessarily reflect the quality of the algorithms themselves, but rather the performance
of the implementations when used with Rust's hasher infrastructure. 

I would like to bench different workloads in the future (everything has been set up to enable this generically).
