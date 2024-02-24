# Stock-Tracking CLI with Async Streams

[Two-Project Series: Build a Stock-Tracking CLI with Async Streams in Rust](https://www.manning.com/liveprojectseries/async-streams-in-rust-ser)

- [Project 1: Data Streaming with Async Rust](https://www.manning.com/liveproject/data-streaming-with-async-rust)
- [Project 2: Advanced Data Streaming with Async Rust](https://www.manning.com/liveproject/advanced-data-streaming-with-async-rust)

## The Original Description
In this liveProject, you’ll use some of the unique and lesser-known features of the Rust language to finish a prototype of a FinTech command line tool.  
You’ll step into the shoes of a developer working for Future Finance Labs, an exciting startup that’s aiming to disrupt mortgages, and help out with building its market-tracking backend.  
Your CTO has laid out the requirements for the minimum viable product, which must quickly process large amounts of stock pricing data and calculate key financial metrics in real time.

## Notes

## The Most Notable Crates Used

## Running the App
### Example
```shell
$ cargo run -- --from 2023-07-03T12:00:09Z --symbols AAPL,AMD,AMZN,GOOG,KO,LYFT,META,MSFT,NVDA,UBER

period start,symbol,price,change %,min,max,30d avg
2023-07-03T12:00:09+00:00,AAPL,$182.52,-4.79%,$166.46,$197.86,$187.24
2023-07-03T12:00:09+00:00,AMD,$176.52,52.41%,$93.67,$181.86,$170.15
2023-07-03T12:00:09+00:00,AMZN,$174.99,34.38%,$119.57,$174.99,$163.51
2023-07-03T12:00:09+00:00,GOOG,$145.29,20.51%,$116.87,$154.84,$146.57
2023-07-03T12:00:09+00:00,KO,$61.20,2.64%,$51.97,$62.06,$59.95
2023-07-03T12:00:09+00:00,LYFT,$16.01,57.89%,$9.17,$19.03,$13.68
2023-07-03T12:00:09+00:00,META,$484.03,69.41%,$282.95,$486.13,$427.50
2023-07-03T12:00:09+00:00,MSFT,$410.34,22.14%,$310.93,$419.77,$403.15
2023-07-03T12:00:09+00:00,NVDA,$788.17,85.86%,$403.22,$788.17,$655.10
2023-07-03T12:00:09+00:00,UBER,$78.20,81.48%,$40.62,$81.39,$69.35
```

## Potential Improvements and Additions
