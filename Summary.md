# Importance to project

- Actors are a different way of unifying and linking processing steps. As a programming pattern that is well supported
  by a framework it greatly facilitates building such pipelines with the ability to run across threads (or even multiple
  computers), thus providing a great way of scaling out any application.
- By using a framework that takes care of the messaging, synchronization, and instantiation, you can focus on business
  logic and data processing steps to wire them up later. This type of encapsulation also leads to code that simply works
  on the input data structure and can be placed anywhere in the pipeline without much effort. Additionally, actors can
  handle state so keeping an open file handle or connection to an external service is easily possible as well. Building
  this code by hand using thread-pools, channels, etc. would require a lot more time and knowledge - similar to using a
  raw TCP stream vs a web framework.
- Connecting actors internally is fairly trivial. However, in order to access an actor’s state from outside the actor
  system, a little redesign is necessary - especially when there are multiple actors processing a single message type.
  As the developer you have to decide what data structure to use (lock-free or locking), which executor to spawn the
  webserver with, and how to retrieve the data from the collection actor.
- Whichever approach you pick, it will have a significant impact on performance since locking a thread with a mutex may
  block multiple actors from executing and could even lead to a deadlock! Solving this case requires a good
  understanding of the requirements: can the data be a little (i.e. seconds) outdated? Is cloning the data feasible? Is
  a single actor able to handle the number of incoming messages? For this project, anything workable will do - so
  use the chance to try out different things and see how it goes.

# Project Conclusions

- Rust is a highly dynamic ecosystem that is quickly evolving to span the gap between systems programming and higher
  level tasks like data processing or web applications.
- Although Rust comes with language support for `async`/`await`, using it in practice requires carefully evaluating
  crates
  for their compatibility and maturity.
- `async` programming allows to do lightweight task switching instead of blocking the CPU scheduler with long-running
  operations. `await` can indicate a possibility for a task switch. This enables tasks to be scheduled tightly and
  increase efficiency. Consequently, you should use `await` whenever possible!
- Data de-duplication and validation is work that can be done in a different application. Not only may this be easier in
  languages like `SQL` but also allows for fine-grained scaling.
- Robustness is an important thing in standalone applications: will the program simply panic on the first error? What
  happens to actors that crash? Is the data safe? `unwrap()` (or better: `expect()`) should only be used if the
  application
  can neither retry nor continue.
- Getting dependencies right is a large part of any Rust project. APIs change, maintainers change, and the language
  evolves. With the Rust team’s lean approach to the standard library, it’s important to avoid using multiple versions
  of the same crate or multiple crates with the same goal.
- Processing streaming data is significantly easier with the actor pattern, while Rust’s ownership model lets you retain
  full control over memory copies.
- Logging should always be an integral part of a complex application.  
  Crates like [log4rs](https://crates.io/crates/log4rs) allow a pluggable and configurable log mechanism to use in Rust.
- Always think of your future self when writing code: will you be able to quickly make sense of what you see in a year?
  While it may be less efficient in terms of runtime, it’s certainly more efficient in time spent understanding what’s
  going on.
