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
