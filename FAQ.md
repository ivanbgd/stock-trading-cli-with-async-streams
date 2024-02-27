# FAQ

## What `async` runtime is the best?

There are many to choose from and if they have a certain maturity, the performance differences should be negligible (for
this project, anyway).  
What remains are two major things to consider: compatibility to other frameworks and API stability/maintenance.  
The former is important because some other libraries come with hard dependencies on - for
example - [tokio](https://crates.io/crates/tokio), so if you picked [Bastion](https://crates.io/crates/bastion), you’d
end up with two async executors and potentially strange side effects.  
Both are personal or project specific choices - and for this project it is highly recommended to
use [async-std](https://crates.io/crates/async-std), which provides an abstraction layer over several runtimes (incl.
its own runtime).

*Note*: Since the crate [hyper](https://crates.io/crates/hyper) requires the `tokio` runtime, this project uses `tokio`
from
within `async-std`.

## Is there a specific edition/version of Rust I should use?

You should use the latest version of Rust whenever possible.

## What are the advantages in making traits `async`?

Traits can be used as interfaces to provide a production and a testing implementation of something (for example) - and
for modern Rust, this interface can require `async` functions.

Until Rust 1.75 `async` traits were not supported out of the box and required using an additional
crate: [async_trait](https://crates.io/crates/async-trait).

As of Rust 1.75, `async` traits are
supported ([see Rust Blog](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits.html)).

## I’m used to seeing `async` calls on blocking operations as network or database queries, not regular processing. Does Rust `async` code make better usage of multi-core processors?

CPU-bound applications typically don’t benefit from `async`, although the runtime is multithreaded across different
cores.  
[crossbeam](https://crates.io/crates/crossbeam) or other crates help with effective parallel processing.  
Tasks that benefit from `async` are usually I/O bound and require lots of waiting, such as network or disk I/O.  
In the case of this project (series) the calculations are light and using `async` to schedule them on a loop can
increase efficiency since we are frequently doing a lot of them, so our cores won’t be idle or overloaded.

If you are considering some heavier processing, check out [crossbeam](https://crates.io/crates/crossbeam) or other
crates that provide parallelization features.

## How can I test `async` code?

Tests require, just as regular code, a runtime to schedule and execute futures.  
As `async` functions, the return type will always be a `Future` type, thus the test runner (`cargo` in this case) has to
be able to execute those.  
Use an attribute such as `tokio::test`, or `async_std::test` from the framework of your choosing (`async-std` being the
default).
