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

- The goal is to fetch all S&P 500 stock data from the [Yahoo! Finance API](https://finance.yahoo.com/).
- Data is fetched from the date that a user provides as a CLI argument to the current moment.
- Users also provide symbols that they want on the command line.
- Extracted data are minimum, maximum and closing prices for each requested symbol, along with percent change and a
  simple moving average as a window function (over the 30-day period).

## Implementation Notes

- We started with a regular ("sequential") async version.
    - With all S&P 500 symbols it took 84 seconds for it to complete.
- Then we upgraded the main loop to use explicit concurrency with async-await paradigm.  
  It runs multiple instances of the same `Future` concurrently.  
  This uses the waiting time more efficiently.  
  This can increase the program's I/O throughput dramatically, which may be needed to keep the strict schedule with an
  async stream (that ticks every 10 seconds), without having to manage threads or data structures to retrieve results.  
  We fetch the S&P 500 data every 10 seconds.
    - This way it takes the program less than two seconds to fetch all S&P 500 data, instead of 84 seconds as before, on
      the same computer and at the same time of the day.
    - That's a speed-up of around 50 times on the computer.
    - This further means that we can fetch data even more frequently than 10 seconds.

## Notes on Data

- List of S&P 500 symbols (tickers) can be found at:
    - https://github.com/datasets/s-and-p-500-companies/blob/main/data/constituents.csv
    - https://en.wikipedia.org/wiki/List_of_S%26P_500_companies
- The alphabetically-sorted list is provided in [sp500_feb_2024.csv](sp500_feb_2024.csv).
    - Symbols "BF.B" and "BRK.B" are removed from the list.
    - There are 501 symbols in it.
- The output is not in the same order as input because of concurrent execution of futures.

## The Most Notable Crates Used

- [async-std](https://async.rs/), as an async library
- [clap](https://crates.io/crates/clap), for CLI arguments parsing
- [futures](https://crates.io/crates/futures), for an implementation of futures
- [time](https://crates.io/crates/time), as a date and time library (used by `yahoo_finance_api`)
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
