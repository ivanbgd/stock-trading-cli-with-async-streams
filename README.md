# Stock-Tracking CLI with Async Streams

[Two-Project Series: Build a Stock-Tracking CLI with Async Streams in Rust](https://www.manning.com/liveprojectseries/async-streams-in-rust-ser)

- [Project 1: Data Streaming with Async Rust](https://www.manning.com/liveproject/data-streaming-with-async-rust)
- [Project 2: Advanced Data Streaming with Async Rust](https://www.manning.com/liveproject/advanced-data-streaming-with-async-rust)

## The Original Description

In this liveProject, you’ll use some of the unique and lesser-known features of the Rust language to finish a prototype
of a FinTech command line tool.

You’ll step into the shoes of a developer working for Future Finance Labs, an exciting startup that’s aiming to disrupt
mortgages, and help out with building its market-tracking backend.

Your CTO has laid out the requirements for the minimum viable product, which must quickly process large amounts of stock
pricing data and calculate key financial metrics in real time.

## Additional Description

- The goal is to fetch all [S&P 500 stock data](https://www.marketwatch.com/investing/index/spx) from
  the [Yahoo! Finance API](https://finance.yahoo.com/).
    - We are using the [yahoo_finance_api](https://crates.io/crates/yahoo_finance_api) crate for this purpose, and it
      allows for both blocking and asynchronous way of work.
    - [The official S&P 500 web page](https://www.spglobal.com/spdji/en/indices/equity/sp-500/#overview)
- Data is fetched from the date that a user provides as a CLI argument to the current moment.
- Users also provide symbols (tickers) that they want on the command line.
- The fetched data include OLHC data (open, low, high, close prices), timestamp and volume, for each symbol.
- The data that we extract from the received data are minimum, maximum and closing prices for each requested symbol,
  along with percent change and a simple moving average as a window function (over the 30-day period).
- As can be seen, the calculations performed are not super-intensive.
- We are implementing **MUCH MORE** than is required of us in the project description!
- The goal is to experiment:
    - with single-threaded synchronous (blocking) code,
    - with multithreaded synchronous (blocking) code,
    - with single-threaded asynchronous code,
    - with multithreaded asynchronous code,
    - with different implementations of Actors ([the Actor model](https://en.wikipedia.org/wiki/Actor_model)):
        - with [actix](https://crates.io/crates/actix), as an Actor framework for Rust,
        - with [xactor](https://crates.io/crates/xactor), as another Actor framework for Rust,
        - with **our own** asynchronous implementation of Actors,
        - TODO/SKIP: with **our own** synchronous implementation of Actors (the problem is I/O-bound),
        - with a single actor that is responsible for downloading, processing, and printing of the processed data to
          console,
        - with two actors: the data-fetching one and the one that combines data processing and printing of results,
        - with three actors: one for data-fetching, one for data-processing and printing to console, and one for writing
          results to a `CSV` file,
        - with `async/await` combined with Actors,
        - with the Publisher/Subscriber model for Actors,
        - with single-threaded implementation,
        - with multithreaded implementation (with various libraries),
        - with various combinations of the above.
- Some of that was suggested by the project author, and some of it (a lot of it) was added on own initiative.
    - Not everything is contained in the final commit.
    - The repository's commit history contains all those different implementations.
- The goal was also to create a web service for serving the requests for fetching of the data.
    - We can send the requests manually from the command line. We are not implementing a web client.
- We measured execution time for various implementations.
    - Some of those results are mentioned in this document, and some are present in code,
      in [logic.rs](./src/logic.rs) - it contains newer information.
    - All time measurements were performed in the *DEBUG* build profile.
    - The *RELEASE* build profile was also tried out, but it didn't yield much performance improvement
      over the *DEBUG* profile, if any at all.

## Implementation Notes and Conclusions

Here follow implementation details and conclusions.

We also included comparison of different implementations.

### Synchronous Single-Threaded Implementation

- We started with synchronous code, i.e., with a blocking implementation.
- The implementation was single-threaded.
- The [yahoo_finance_api](https://crates.io/crates/yahoo_finance_api) crate that we use supports the blocking feature.
- This implementation was slow.
- Writing to file was not implemented at this stage.

### Asynchronous Single-Threaded Implementation

- Then we moved to a regular (sequential, single-threaded) `async` version.
    - With all S&P 500 symbols it took `84` seconds for it to complete.
- Then we upgraded the main loop to use explicit concurrency with `async/await` paradigm.  
  It runs multiple instances of the same `Future` concurrently.  
  We are using the [futures](https://crates.io/crates/futures) crate to help us achieve
  this: `futures::future::join_all(queries).await;`.  
  This uses the waiting time more efficiently.  
  This is not multithreading.  
  This can increase the program's I/O throughput dramatically, which may be needed to keep the strict schedule with an
  async stream (that ticks every `n` seconds, where `n = 30` by default), without having to manage threads or data
  structures to retrieve results.  
  We fetch the S&P 500 data every `n` seconds.
    - This way it takes the program less than `2` seconds to fetch all S&P 500 data, instead of `84` seconds as
      before, on the same computer and at the same time of the day.
    - That's a speed-up of around 50 times on the computer.
    - This further means that we can fetch data a lot more frequently than the default `30` seconds.
- Using the same explicit concurrency with `async/await` paradigm but with configurable *chunk* size gives more or less
  the same execution time. This is the case in which our function `handle_symbol_data()` still processes one symbol, as
  before.
    - Namely, whether chunk size of symbols is equal 1 or 128, all symbols are processed in around `2.5` seconds.
    - The function `handle_symbol_data()` is a bottleneck in this case.
- If we modify `handle_symbol_data()` to take and process multiple symbols at the time, i.e., to work with *chunks* of
  symbols, the execution time changes depending on the chunk size.
    - For example, for chunk size of 1, it remains `~2.5` s.
    - For chunk size equal 3, it is around `1.3` s!
    - For chunk size equal 5 or 6, it is around `1.2` s! Chunk size of 5 seems like a sweet-spot for this application
      with this implementation.
    - For chunk size equal 2 or 10, it is around `1.5` s.
    - For chunk size equal 128, it rises to over `13` s!
- Thanks to the fact that the calculations performed on the data are not super-intensive, we can conclude
  that `async/await` can be fast enough, even though it's generally mostly meant for I/O.
    - Namely, the calculations are light, and we are doing a lot of them frequently.
      We have around 500 symbols and several calculations per symbol.
      The data-fetching and data-processing functions are all asynchronous.
      We can conclude that scheduling a lot of lightweight async tasks on a loop can increase efficiency.
    - Fetching data from a remote API is an I/O-bound task, so it is all in the relative ratio between fetching and
      processing of the data.
    - It is probably the case that the I/O part dominates the CPU part in this application, and consequently
      the `async/await` paradigm is a good choice in this case.
- We are using `async` streams for improved efficiency (`async` streaming on a schedule).
- Unit tests are also asynchronous.
- Writing to file was not implemented at this stage.

### Asynchronous Multithreaded Implementation

- Using the [rayon](https://crates.io/crates/rayon) crate for parallelization keeps execution time at around `2.5`
  seconds without chunking symbols.
    - We still use `futures::future::join_all(queries).await;` to join all tasks.
- Using `rayon` with chunks yields the same results as above implementation with chunks.
    - For chunk size equal 5, execution time is around `1.0` s! Chunk size of 5 again seems like a sweet-spot for this
      application with this implementation.
    - All CPU cores are really being employed (as can be seen in Task Manager).
- Using `rayon` is easy. It has parallel iterators that support *chunking*. We can turn our sequential code into
  parallel by using `rayon` easily.
- We also implemented classical multithreading with `tokio::spawn()`.
    - This requires using the crate [once_cell](https://crates.io/crates/once_cell) and its
      struct `once_cell::sync::OnceCell`.
      It is needed to initialize the variable `symbols` that holds symbols that a user provides on the command line.
    - Starting with [Rust 1.70.0](https://blog.rust-lang.org/2023/06/01/Rust-1.70.0.html#oncecell-and-oncelock),
      we can use the standard library's [std::sync::OnceLock](https://doc.rust-lang.org/std/sync/struct.OnceLock.html)
      instead of `once_cell::sync::OnceCell` with same results.
        - The functionality has been ported from the crate to the standard library.
    - *Note*: This implementation doesn't employ `rayon`.
    - Performance is the same as with explicit concurrency with `async/await` or with `rayon`.
        - The sweet spot for the chunk size is again 5, and it yields execution time of `1.2` s.
- Wrapping `symbols` in `std::sync::Arc` and using `std::thread::scope` provides a working solution that is almost as
  fast as other fast multithreading solutions.
- We can conclude that `rayon` and `Tokio` provide equal performance in our case, and that the standard library
  MT solution is more or less of the same speed in our case.
- A higher CPU utilization can be observed with chunk size of 5 than with chunk size of 128, for example, which is good,
  as it leads to higher efficiency.
- All measurements were performed with the 505 S&P symbols provided.
    - Comments in code also assume all 505 symbols.
    - The official list of tickers changes from time to time, so our list can be updated accordingly
      (see the *Notes on Data* section below).
- If only 10 symbols are provided, instead of 505, then the fastest solution is with chunk size of 1, around `250` ms.
    - Chunk size of 5 is slower, around `600` ms.
    - Chunk sizes of 10 or 128 are very slow, over `1` s!
- We are probably limited by the data-fetching time from the Yahoo! Finance API. That's probably our bottleneck.
    - We need to fetch data for around 500 symbols and the function `get_quote_history()` fetches data for one symbol
      (called "ticker") at a time. It is asynchronous, but perhaps this is the best that we can do.
- Writing to file had not been implemented at this stage.
- When we added writing to file, using `rayon` with chunk size equal 5 yielded execution time of `1.0`!
- With writing to file, using `Tokio` with chunk size equal 5 yields execution time of `0.9` s!
- TODO: With writing to file, `stdlib` with chunk size equal 5 yields execution time of ??? s!
- TODO/SKIP: With writing to file, using `crossbeam` with chunk size equal 5 yields execution time...
- We used a barrier to write all results, from all threads, to the file at once.
- TODO: We also implemented writing individual chunks, by individual threads, to the file, by using a lock.
    - TODO: Performance was the same as with barrier for writing all results at once. ?! CHECK!!!

### Synchronous Multithreaded Implementation: TODO

- TODO: Using `rayon` with chunk size equal 5 yields execution time equal `1` second!
- TODO: Using `crossbeam` with chunk size equal 5 yields execution time...
    - `crossbeam` is best suited to CPU-bound tasks, while our task is I/O bound, so we could skip it this time around.
- TODO: Using `stdlib` threading with chunk size equal 5 yields execution time...
- Writing to file was implemented at this stage. It requires synchronization, as multiple threads write to the same
  file.
    - We can write chunks, which requires using a lock, or we can write all results at once, which requires a barrier.
        - Writing chunks makes more sense with large amounts of data, especially if they don't fit in memory.
        - Writing all results at once probably makes more sense when the amount of data is not large and fits in memory,
          which is our case.

### Asynchronous Multithreaded Implementation with Actors

- We introduced `Actors` ([the Actor model](https://en.wikipedia.org/wiki/Actor_model)).
- This can be a fast solution to this problem, even with one type of actor that performs all three operations
  (fetch, process, write), but it depends on implementation a lot.
- Having only one Actor doesn't make sense, but we started with one with the intention to improve from there.
- We initially kept the code asynchronous.
    - It was on the order of synchronous and single-threaded `async` code, i.e., `~80-90` seconds, when actors were
      processing one symbol at a time (when a request message contained only one symbol to process). This applies in
      case of the `actix` crate with a single `MultiActor` that does all three operations., and in case of three
      actors when using the `xactor` crate.
    - When we increased the chunk size to 128, the `MultiActor` performance with `actix` improved a lot, enough for
      it to fit in the 30-second window.
    - Interestingly, reducing the chunk size back to 1 now, in this implementation, was able to put the complete
      execution in a 5-second slot, possibly in even less than `3` s.
    - Making chunk size equal 5 or 10 reduced execution time to `1.5-2` s.
- We then moved on to a Two-Actor asynchronous implementation.
    - One actor was responsible for fetching data from the Yahoo! Finance API, and the other for processing
      (calculating) performance indicators and printing them to `stdout`.
    - We only implemented actors that process one symbol at a time, i.e., not in chunks.
    - We tried with a single instance of the `FetchActor` and multiple instances of `ProcessorWriterActor`, as well
      as with multiple instances of both types of Actor. The number of instances was equal to the number of symbols.
        - In either case, `FetchActor` spawns `ProcessorWriterActor` and sends it messages with fetched data.
        - The two cases have the same performance, which is around `2.5` s.
    - We tried without and with `rayon`.
        - Neither implementation is ordered, meaning output is not in the same order as input.
        - The `rayon` implementation uses multiple instances of both types of Actor, and has the same performance as
          the non-`rayon` implementation, i.e., `~2.5` s.
- The Three-Actor implementation has the `ProcessorWriterActor` split into two actors, for the total of three
  actor types.
    - The `FetchActor` is responsible for fetching data from the Yahoo! Finance API.
    - The `ProcessorActor` calculates performance indicators and prints them to `stdout`.
    - The `WriterActor` writes the performance indicators to a `CSV` file.
    - As for performance, the execution time is around `2.5` s without chunks.
        - Each actor works with a single symbol.
        - There are as many `FetchActor`s and `ProcessorActor`s as there are symbols.
    - If we introduce chunks, the performance increases.
        - We worked with chunk size equal 5.
        - There are as many `FetchActor`s and `ProcessorActor`s as there are chunks.
            - The execution time was possibly below `2` seconds.
        - If the `WriterActor` also works with chunks (of the same size and with the same amount of them), the execution
          time is again below `2` seconds, but possibly even shorter than in the previous case, i.e., around `1.5` s,
          making it a very fast solution.
            - This includes flushing of the buffer to file with every chunk, which makes it possible to write all rows
              to the file, and to still get an even better performance in terms of execution speed.
            - We are using `std::io::BufWriter` to improve the write performance. It wouldn't make sense to flush the
              buffer unless we worked with chunks of symbols, i.e., it wouldn't make sense to flush it for a single
              symbol, although, that would make the solution correct because it would write all rows to the file.
              Still, to have both good performance and a correct solution, we should use chunks and flush the buffer
              for every chunk.
        - Using `rayon` is at least equally fast, but probably not faster.
    - This implementation writes to a file, unlike previous implementations, so it is expected that its performance
      is slightly worse because of that.
    - With async code it was not possible to have the `WriterActor` write out all ~500 rows, i.e., performance
      indicators for all symbols, in the file, if we only flushed when stopping the actor, i.e., in its `stopped`
      method.
        - Namely, we stop the main loop, which is infinite, by interrupting program by pressing `CTRL+C`, so
          the `stopped` method doesn't have a chance to get executed and flush the buffer.
        - Increasing the `WriterActor`'s mailbox size didn't help.
        - The course-author's `xactor`-based solution also wasn't able to write all rows to the file, but they only
          flush the buffer in the actor's `stopped` method.
    - By adding flushing of the buffer to the file in the `WriterActor`'s `handle` method, we are able to solve this
      issue.
        - We do this in case the `WriterActor` also works with *chunks*.
    - All ~500 rows do get printed to `stdout` regardless of the `WriterActor`, as the output to console is handled by
      the `ProcessorActor`.
- We experimented with the Publisher/Subscriber model with the `Actix` framework, to get a feel of it.
    - The Publisher/Subscriber model is generally better suited to applications that have a number of different
      instances of the same actor, and they should all get a message, or where there are different types of subscriber
      actors which should all receive the same message, i.e., be notified of it.
    - Our application is not like that, but we still wanted to try it out and play around with it a little.
        - Namely, we don't want our actors to do the double work, so we have only one instance of each.
        - We could try to implement multiple instances of actors for fun, to see if all of them really get the messages.
            - For some more fun, we could randomize sending messages to the only instance of each type of actor. They
              wouldn't be doing double work in that case, which would make the implementation correct.
    - We weren't able to make it work.
        - The issue is that some of our actors are both publishers and subscribers at the same time, and perhaps Actix
          simply doesn't support that, or at least not with async functions.
        - Perhaps it can be solved, but I am not sure. Check out:
            - https://github.com/actix/actix/blob/master/actix/examples/ring.rs - All Nodes in the example are of the
              same type, but this doesn't seem like a Publisher/Subscriber model. They create a node from another node
              to form a ring (a full circle) of nodes, but that's not P/S.
            - The [actix-broker](https://crates.io/crates/actix-broker) crate.
                - https://docs.rs/actix-broker/latest/actix_broker/
                - https://github.com/actix/actix/blob/master/actix-broker/examples/basic.rs
                - Couldn't get it to work because of `async` blocks and lifetimes
                  ([commit](https://github.com/ivanbgd/stock-trading-cli-with-async-streams/tree/e3af311363430c2c3a668025627805d9a5c8258d)).
    - Performance: N/A
- We are using [actix](https://crates.io/crates/actix) as an Actor framework for Rust, and
  its [actix-rt](https://crates.io/crates/actix-rt) as a runtime.
    - *Note*: [actix-rt](https://crates.io/crates/actix-rt) is a "Tokio-based single-threaded async runtime for the
      Actix ecosystem".
- We implemented **our own** asynchronous Actor model from scratch.
    - We have a variant with a universal (general) actor that can receive and handle multiple message types.
    - We have a variant of specific actors that can only process and handle a single, specific, message type.
    - We use [Tokio](https://tokio.rs/) as asynchronous runtime.
    - Our implementation is project-specific - a custom one.
        - It is not general. It is not meant as a general library, but it could be generalized. It's a good starting
          point for a general Actor Model library.
    - Performance is excellent! It is at least as fast as our previous fastest solution, if not faster.

### Synchronous Multithreaded Implementation with Actors: TODO

- Our task is I/O bound, so we could skip this.

### The Web Service

- The actors are connected to the outside world.
- We create a web service for this.

## Additional Explanation

Check out the files that are provided for additional explanation:

- [FAQ](FAQ.md)
- [Summary](Summary.md)

Most of those were provided by the course author, but were modified where it made sense.

## Notes on Data

- List of S&P 500 symbols (tickers) can be found at:
    - https://en.wikipedia.org/wiki/List_of_S%26P_500_companies
    - https://github.com/datasets/s-and-p-500-companies/blob/main/data/constituents.csv
    - The lists are not necessarily up-to-date.
- The alphabetically-sorted list is provided in [sp500_2024_aug.csv](sp500_2024_aug.csv).
    - There are 505 symbols in it, so ~500.
    - Keep in mind that some symbols come and go to/from the S&P 500 list.
        - In case a symbol is not officially on the list, it will be ignored and consequently
          not shown in the output, be it in `stdout` or in the generated `output.csv`.
        - The official list should be looked up from time to time and the input CSV file in this repository,
          that contains the list, should be updated accordingly.
            - The file's name intentionally contains the date when it was constructed.
- The output is not in the same order as input because of concurrent execution of futures.

## Notes on Code

### Async Executors/Runners

- Use `#[tokio::main]` for most of the code variants.
- Use `#[actix::main]` or `#[actix_rt::main]` if working with the `actix` actor framework.
- Use `#[xactor::main]` if working with the `xactor` actor framework.
- Try `#[async_std::main]` in some variants, too.

## The Most Notable Crates Used

Not all of them are present in every commit.  
The `git` commit history contains descriptive comments.

- [actix](https://crates.io/crates/actix), as an Actor framework for Rust
- [actix-rt](https://crates.io/crates/actix-rt), as Tokio-based single-threaded async runtime for the Actix ecosystem
- [async-std](https://async.rs/), as an async library
- [clap](https://crates.io/crates/clap), for CLI arguments parsing
- [futures](https://crates.io/crates/futures), for an implementation of futures (required for explicit concurrency
  with `async/await` paradigm)
- [rayon](https://crates.io/crates/rayon), as a data-parallelism library for Rust
- [time](https://crates.io/crates/time), as a date and time library (used by `yahoo_finance_api`)
- [Tokio](https://tokio.rs/), as an asynchronous runtime - used both directly and as a dependency of some other crates
- [xactor](https://crates.io/crates/xactor), as a Rust Actors framework based on async-std (it also supports Tokio as
  runtime instead of async-std, but we didn't use that)
- [yahoo_finance_api](https://crates.io/crates/yahoo_finance_api), as an adapter for
  the [Yahoo! Finance API](https://finance.yahoo.com/) to fetch histories of market data quotes

## Running the App

- Help is available, via `--help` or `-h` option.
- The application requires the `from` and the `symbols` arguments.
- The `from` date and time argument should be provided in the [RFC3339](https://datatracker.ietf.org/doc/html/rfc3339)
  format.
- The `symbols` argument should contain comma-separated S&P symbols (tickers), with no blanks between them.
- The `to` date and time are assumed as the current time instant, at the moment of execution of each iteration of the
  loop (at each new interval).
- The examples below demonstrate how to run the app.
- The output date and time are also in the `RFC3339` format.
- The program runs in a loop with a specified interval (in [src/constants.rs](src/constants.rs)).
- `cargo` also catches the `CTRL+C` signal, which interferes with this application's catching of the signal,
  and this can be problematic only on Windows, as discussed
  [here](https://www.reddit.com/r/rust/comments/6lsead/problems_with_ctrlc_handling_under_rust_in_windows/).
  The solution is to run the binary directly and not through `cargo`.
- Since **tracing** is provided, you can enable the tracing output by `export RUST_LOG=INFO`, or `DEBUG`, etc.
- The `variant` option is available for deciding between my implementation of actors (default), Actix implementation
  of actors, or implementation without actors - all those with or without rayon (default); see help.
    - This is used for easier testing and timing, as we only have to build once this way.

### Example 1: Provide Some Symbols On the Command Line

```shell
$ cargo run -- --from 2023-07-03T12:00:09+00:00 --symbols AAPL,AMD,AMZN,GOOG,KO,LYFT,META,MSFT,NVDA,UBER

*** 2024-02-27 19:42:58.0795392 +00:00:00 ***

period start,symbol,price,change %,min,max,30d avg
2023-07-03T12:00:09Z,AMD,$177.65,53.38%,$93.67,$181.86,$172.12
2023-07-03T12:00:09Z,GOOG,$140.12,16.23%,$116.87,$154.84,$146.27
2023-07-03T12:00:09Z,LYFT,$16.63,64.00%,$9.17,$19.03,$13.90
2023-07-03T12:00:09Z,META,$485.17,69.63%,$283.25,$486.13,$435.32
2023-07-03T12:00:09Z,UBER,$78.10,81.25%,$40.62,$81.39,$70.34
2023-07-03T12:00:09Z,NVDA,$791.87,86.70%,$403.26,$791.87,$671.35
2023-07-03T12:00:09Z,AMZN,$173.62,33.33%,$119.57,$174.99,$164.80
2023-07-03T12:00:09Z,MSFT,$407.20,20.48%,$312.14,$420.55,$405.11
2023-07-03T12:00:09Z,AAPL,$183.13,-4.85%,$166.89,$198.11,$187.16
2023-07-03T12:00:09Z,KO,$60.17,-0.67%,$52.38,$63.05,$59.97

Took 278.264ms to complete.
```

New symbols (tickers) added in 2024:

```shell
$ cargo run -- --from 2024-01-01T12:00:09+00:00 --symbols KKR,CRWD,GDDY,VST,GEV,SOLV,SMCI,DECK
```

Include a ticker that doesn't exist, BBB, for testing purposes:

```shell
$ cargo run -- --from 2024-01-01T12:00:09+00:00 --symbols AAPL,AMZN,BBB,GOOG,MSFT
```

### Example 2: Provide All Symbols From a File

```shell
$ cargo run -- --from 2024-07-03T12:00:09+00:00 --symbols "$(cat sp500_2024_aug.csv)"
```

Or, equivalently:

```shell
$ export SYMBOLS="$(cat sp500_2024_aug.csv)" && cargo run -- --from 2024-07-03T12:00:09+00:00 --symbols $SYMBOLS
```

## Conclusion

- This application fetches data from a remote API, so it is relatively **I/O-bound**.
- There are some light calculations taking place, but I believe that the application is more on the I/O-bound side.
- The `async/await` paradigm copes well with this kind of application.
- We also implemented the **Actor Model**. It uses message passing between actors.
    - Actors slow down execution a little, but not too much in case of this application.
    - They work quite nicely and can be configured and customized to our liking.
    - We were able to use third-party Actor frameworks, as well as our own implementation.
        - Actix and our implementation didn't show much difference in performance in this application, if any.
    - Actors can be combined successfully with `rayon`.
- Working with **chunks** of data instead of with individual pieces of data improves performance.
    - Not all chunk sizes perform the same, so we need to find a sweet spot - an optimal chunk size.
    - The optimal chunk size depends on implementation in general, but in our case it didn't vary much,
      if at all, between different implementations.
- It probably helps even more so in a distributed setting (a distributed network of nodes) to send larger messages,
  than smaller ones, in both ways, which means to work with **chunks** instead of individual symbols.
- We couldn't measure execution time 100% properly in case of Actors with asynchronous code, but it's close.
    - Namely, we send `start` time in a message and ultimately forward it to the Writer Actor which
      then calculates and prints the execution time after it has written results to the file in each iteration,
      but, it does so on a per-chunk basis, which slows execution down a little,
      and doesn't look quite nice in the output.
    - Writing to file happens on a per-chunk basis regardless of time measurement, so it's just the multiple
      printouts, instead of a single printout per iteration, that slow down execution just a little bit, and it
      doesn't look quite nice, but it means to us to measure execution time, so we can neglect those
      small inconveniences.
- The fastest solution is asynchronous (using `tokio`) without actors, but very close to it is asynchronous solution
  using `rayon` without actors.
- **Graceful shutdown** is implemented by means of waiting (sleeping) for some time in the `main()` function,
  giving the running tasks time to finish.

## Potential Modifications, Improvements or Additions

- Find ways to **publish and subscribe** to messages without explicit calls.
    - [actix](https://crates.io/crates/actix) might support this feature through the use
      of [Recipient](https://docs.rs/actix/latest/actix/struct.Recipient.html);
      also [see here](https://actix.rs/docs/actix/address#recipient).
        - [We tried to do this](https://github.com/ivanbgd/stock-trading-cli-with-async-streams/tree/cff6a85614817673ca41987ba3f8f8049a3b1df0),
          to no avail. Perhaps we are missing something, or perhaps Actix doesn't allow actors to be both a publisher
          and a subscriber at the same time, which is what we need.
    - [xactor](https://crates.io/crates/xactor) does support the feature, but at the time of this writing, it hadn't
      been updated in about three years. Also, `actix` is far more popular.
        - Still, a `xactor` implementation with three different Actors and with publish/subscribe model can be found in
          [this commit](https://github.com/ivanbgd/stock-trading-cli-with-async-streams/commit/d4f53a7499ef9ceee988a6e3d5d26d518e25f6eb).
- Try implementing **graceful shutdown** by using
  [CancellationToken](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)s
  and/or [TaskTracker](https://docs.rs/tokio-util/latest/tokio_util/task/task_tracker/struct.TaskTracker.html)
  that we'd send to all actors (perhaps `WriterActor` doesn't need it).
    - Actors complicate this a little, and our current solution works well - it's fully-graceful.
    - Actors or no Actors, we are working with chunks of data,
      so do we have to handle `CTRL+C` all the way down to the `calculate()` functions in
      [async_signals.rs](./src/async_signals.rs)?
    - Namely, we don't want to have some symbols processed and some omitted. If we start fetching and processing
      symbols in a new iteration, we'd like to have them all processed.
- Add tracing or at least logging.
- Read symbols from a file instead of from the command line.
- Sort output by symbol.
- Rename "output.csv" to "<output_current_date_and_time_with_tz>.csv".
    - Use RFC2822, or RFC3339, or ISO 8601 or any other format.
    - We can just pass `OffsetDateTime::now_utc()`, which is in UTC TZ.
