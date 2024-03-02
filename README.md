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
      allows for asynchronous way of work.
- Data is fetched from the date that a user provides as a CLI argument to the current moment.
- Users also provide symbols that they want on the command line.
- The fetched data include OLHC data (open, low, high, close prices), timestamp and volume, for each symbol.
- The data that we extract from the received data are minimum, maximum and closing prices for each requested symbol,
  along with percent change and a simple moving average as a window function (over the 30-day period).
- As can be seen, the calculations performed are not super-intensive.
- The goal is to experiment with:
    - synchronous (blocking) code,
    - with single-threaded asynchronous code,
    - with multithreaded asynchronous code (which proved to be the fastest solution for this concrete problem),
    - with different implementations of actors ([the Actor model](https://en.wikipedia.org/wiki/Actor_model)):
        - with [actix](https://crates.io/crates/actix), as an Actor framework for Rust,
        - with [xactor](https://crates.io/crates/xactor), as another Actor framework for Rust,
        - perhaps with own implementation of Actors,
        - with a single actor that is responsible for downloading, processing, and printing of the processed data to
          console,
        - with three actors that are responsible for the three mentioned tasks,
        - with `async/await` combined with `Actors`,
        - with single-threaded implementation,
        - with multithreaded implementation,
        - with various combinations of the above.
- Some of that was suggested by the project author, and some of it was added on own initiative.
    - Not everything is contained in the final commit.
    - Commit history contains different implementations.
- The goal was also to create a web service for serving the requests for fetching of the data.
    - We can send the requests manually from the command line. We are not implementing a web client.

## Implementation Notes and Conclusions

- We started with synchronous code. It was slow.
- Then we moved to a regular (sequential, single-threaded) `async` version.
    - With all S&P 500 symbols it took `84` seconds for it to complete.
- Then we upgraded the main loop to use explicit concurrency with `async/await` paradigm.  
  It runs multiple instances of the same `Future` concurrently.  
  We are using the [futures](https://crates.io/crates/futures) crate to help us do
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
    - This proved to be the fastest solution for this concrete problem.
- Using the same explicit concurrency with `async/await` paradigm but with configurable *chunk* size gives more or less
  the same execution time. This is the case in which our function `handle_symbol_data()` still processes one symbol, as
  before.
    - Namely, whether chunk size of symbols is equal 1 or 128, all symbols are processed in around `2.5` seconds.
- If we modify `handle_symbol_data()` to take and process multiple symbols at the time, i.e., to work with *chunks* of
  symbols, the execution time changes depending on the chunk size.
    - For example, for chunk size of 1, it remains `~2.5` s.
    - For chunk size equal 3, it is around `1.3` s!
    - For chunk size equal 5 or 6, it is around `1.2` s! Chunk size of 5 seems like a sweet-spot for this application
      with this implementation.
    - For chunk size equal 2 or 10, it is around `1.5` s.
    - For chunk size equal 128, it rises to over `13` s!
- Thanks to the fact that the calculations performed on the data are not super-intensive, we can conclude
  that `async/await` can be fast enough, even though it's mostly meant for I/O.
    - Namely, the calculations are light, and we are doing a lot of them frequently.
      We have around 500 symbols and several calculations per symbol.
      The data-fetching and data-processing functions are all asynchronous.
      We can conclude that scheduling a lot of lightweight async tasks on a loop can increase efficiency.
- We are using `async` streams for improved efficiency (`async` streaming on a schedule).
- Unit tests are also asynchronous.
- Using the [rayon](https://crates.io/crates/rayon) crate for parallelization keeps execution time at around `2.5`
  seconds without chunking symbols.
    - We still use `futures::future::join_all(queries).await;` to join all tasks.
- Using `rayon` with chunks yields the same results as above implementation with chunks.
    - For chunk size equal 5, execution time is around `1.2` s! Chunk size of 5 again seems like a sweet-spot for this
      application with this implementation.
    - All CPU cores are really being employed (as can be seen in Task Manager).
- Using `rayon` is easy. It has parallel iterators that support *chunking*. We can turn our sequential code into
  parallel by using `rayon` easily.
- We are probably limited by the data-fetching time from the Yahoo! Finance API. That's probably our bottleneck.
    - We need to fetch data for around 500 symbols and the function `get_quote_history()` fetches data for one symbol
      (called "ticker") at a time. It is asynchronous, but perhaps this is the best that we can do.
- We introduced `Actors` ([the Actor model](https://en.wikipedia.org/wiki/Actor_model)).
    - This can be a fast solution for this problem, even with one type of actor that performs all three operations (
      fetch, process, write), but it depends on implementation a lot.
    - It was on the order of synchronous and single-threaded `async` code, i.e., `~80-90` seconds, when actors were
      processing one symbol at a time (when a request message contained only one symbol to process). This applies in
      case of the `actix` crate with a single `MultiActor` that does all three operations, and in case of three actors
      when using the `xactor` crate.
    - When we increased the chunk size to 128, the `MultiActor` performance with `actix` improved a lot, enough for it
      to fit in the 30-second window.
    - Interestingly, reducing the chunk size back to 1 now, in this implementation, was able to put the complete
      execution in a 5-second slot, possibly even less than `3` s.
    - Making chunk size equal 5 or 10 reduced execution time to `1.5-2` s.
    - *Note*: [actix-rt](https://crates.io/crates/actix-rt) is a "Tokio-based single-threaded async runtime for the
      Actix ecosystem".
- The actors are connected to the outside world.
    - We create a web service for this.

## Additional Explanation

Check out the files that are provided for additional explanation:

- [FAQ](FAQ.md)
- [Summary](Summary.md)

Most of those were provided by the course author, but were modified where it made sense.

## Notes on Data

- List of S&P 500 symbols (tickers) can be found at:
    - https://github.com/datasets/s-and-p-500-companies/blob/main/data/constituents.csv
    - https://en.wikipedia.org/wiki/List_of_S%26P_500_companies
- The alphabetically-sorted list is provided in [sp500_feb_2024.csv](sp500_feb_2024.csv).
    - Symbols "BF.B" and "BRK.B" are removed from the list.
    - There are 501 symbols in it.
- The output is not in the same order as input because of concurrent execution of futures.

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
- [Tokio](https://tokio.rs/), as an asynchronous runtime - not used directly, but as a dependency of some crates
- [xactor](https://crates.io/crates/xactor), as a Rust Actors framework based on async-std
- [yahoo_finance_api](https://crates.io/crates/yahoo_finance_api), as an adapter for
  the [Yahoo! Finance API](https://finance.yahoo.com/) to fetch histories of market data quotes

## Running the App

### Example

```shell
$ cargo run -- --from 2023-07-03T12:00:09+00:00 --symbols AAPL,AMD,AMZN,GOOG,KO,LYFT,META,MSFT,NVDA,UBER

period start,symbol,price,change %,min,max,30d avg


*** 2024-02-27 19:42:58.0795392 +00:00:00 ***

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

## Potential Modifications, Improvements or Additions

- Read symbols from a file instead of from the command line.
- Sort output by symbol.
- Find ways to publish and subscribe to messages without explicit calls.
    - [actix](https://crates.io/crates/actix) might support this feature through the use
      of [Recipient](https://docs.rs/actix/latest/actix/struct.Recipient.html);
      also [here](https://actix.rs/docs/actix/address#recipient).
    - [xactor](https://crates.io/crates/xactor) does support the feature, but at the time of this writing, it hadn't
      been updated in about three years. Also, `actix` is far more popular.
        - Still, a `xactor` implementation with three different Actors and with publish/subscribe model can be found in
          [this commit](https://github.com/ivanbgd/stock-trading-cli-with-async-streams/commit/d4f53a7499ef9ceee988a6e3d5d26d518e25f6eb).
- Find a way to measure time correctly when working with async/await and/or with Actors.
- Add tracing or at least logging.
