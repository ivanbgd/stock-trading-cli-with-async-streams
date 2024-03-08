use std::sync::OnceLock;
use std::time::Instant;

// use actix::Actor;
use actix::{Actor, SyncArbiter};
use actix_rt::System;
use clap::Parser;
// use rayon::prelude::*;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::actors::{FetchActor, QuoteRequestMsg, WriterActor};
// use crate::actors::{handle_symbol_data, WriterActor};
use crate::cli::Args;
use crate::constants::{CSV_HEADER, TICK_INTERVAL_SECS};

/// **The main loop**
///
/// Implemented by using explicit concurrency with async-await paradigm.
///
/// Runs multiple instances of the same `Future` concurrently.
///
/// This uses the waiting time more efficiently.
///
/// This can increase the program's I/O throughput dramatically,
/// which may be needed to keep the strict schedule with an async
/// stream (that ticks every [`TICK_INTERVAL_SECS`] seconds), without
/// having to manage threads or data structures to retrieve results.
pub async fn main_loop() -> Result<(), actix::MailboxError> {
    let args = Args::parse();
    let from = OffsetDateTime::parse(&args.from, &Rfc3339)
        .expect("The provided date or time format isn't correct.");

    // let symbols: Vec<String> = args.symbols.split(",").map(|s| s.to_string()).collect();
    // let chunks_of_symbols: Vec<&[String]> = symbols.par_chunks(CHUNK_SIZE).collect();
    // let chunks_of_symbols: Vec<&[String]> = symbols.chunks(CHUNK_SIZE).collect();

    let symbols: Vec<String> = args.symbols.split(",").map(|s| s.to_string()).collect();
    static SYMBOLS: OnceLock<Vec<String>> = OnceLock::new();
    // // let symbols = SYMBOLS.get_or_init(|| args.symbols.split(",").map(|s| s.to_string()).collect());
    let symbols = SYMBOLS.get_or_init(|| symbols);
    // let chunks_of_symbols: Vec<&[String]> = symbols.chunks(CHUNK_SIZE).collect();
    // let chunks_of_symbols: Vec<&[String]> = symbols.par_chunks(CHUNK_SIZE).collect();

    // // let symbols: Vec<&str> = args.symbols.split(",").collect();
    // // let chunks_of_symbols = symbols.chunks(CHUNK_SIZE);

    // TODO: Spawn multiple `FetchActor`s. Perhaps move down into loop, or see another way - with ctx maybe?
    // let fetch_address = FetchActor.start();
    // let proc_writer_address = ProcessorWriterActor.start();
    // let actor_address = SyncArbiter::start(NUM_THREADS, || MultiActor); // Doesn't work (because of async handler).

    // let file_name = "output.csv".to_string();
    // let mut file = File::create(&file_name)
    //     .unwrap_or_else(|_| panic!("Could not open target file \"{}\".", file_name));
    // let _ = writeln!(&mut file, "{}", CSV_HEADER);
    // let writer = Some(BufWriter::new(file));
    // println!("WriterActor is started.");

    // We need to ensure that we have one and only one `WriterActor` - a singleton.
    // This is because it writes to a file, and writing to a shared object,
    // such as a file, needs to be synchronized, i.e., sequential.
    // We generally don't use low-level synchronization primitives such as
    // locks, mutexes, and similar when working with Actors.
    // Actors have mailboxes and process messages that they receive one at a time,
    // i.e., sequentially, and hence we can accomplish synchronization implicitly
    // by using a single writer actor.
    // let writer_address = WriterActor {
    //     file_name: "output.csv".to_string(),
    //     writer: None,
    // }
    // .start();

    // More than one thread is certainly incorrect, but even with only one thread
    // not everything gets written to the file, so we can't consider that correct either.
    // In fact, not all symbols get printed to stdout, but more do than to the file.
    // This solution is asynchronous (async/await), so that could be the culprit.
    let writer_address = SyncArbiter::start(2, || WriterActor {
        file_name: "output.csv".to_string(),
        writer: None,
    });

    // let mut interval = stream::interval(Duration::from_secs(TICK_INTERVAL_SECS));

    // while let Some(_) = interval.next().await {
    // TODO: uncomment
    // todo: remove the FOR line
    for _ in 0..1 {
        // We always want a fresh period end time, which is "now" in the UTC time zone.
        let to = OffsetDateTime::now_utc();

        // For standard output only, i.e., not for CSV
        println!("\n\n*** {} ***\n", to);

        // A simple way to output a CSV header
        println!("{}", CSV_HEADER);

        let start = Instant::now();

        // NEW WITH ACTORS

        // Without rayon. Not sequential. Multiple `FetchActor`s. 2.3 s

        // We start multiple `FetchActor`s - one per symbol, and they will
        // start the next Actor in the process - one each.
        for symbol in symbols.clone() {
            let fetch_address = FetchActor.start();

            let _ = fetch_address
                .send(QuoteRequestMsg {
                    symbol: symbol.to_string().clone(),
                    from,
                    to,
                    writer_address: writer_address.clone(),
                })
                .await?;
        }

        // TODO: We should block here, somehow, for the writer to have time to write everything.
        // todo: its async handler won't even compile, currently, but non-async is not fully-correct
        // todo: or, it is fully correct, but I need to block

        // With rayon. Not sequential. Multiple `FetchActor`s. ~2.5 s
        // It is not much faster (if at all) than the above solution without rayon.
        // Namely, execution time is not measured properly in this case.

        // // We start multiple `FetchActor`s - one per symbol, and they will
        // // start the next Actor in the process - one each.
        // let queries: Vec<_> = symbols
        //     .par_iter()
        //     .map(|symbol| async {
        //         FetchActor
        //             .start()
        //             .send(QuoteRequestMsg {
        //                 symbol: symbol.to_string(),
        //                 from,
        //                 to,
        //                 writer_address: writer_address.clone(),
        //             })
        //             .await
        //     })
        //     .collect();
        // let _ = futures::future::join_all(queries).await;

        // OLD WITH ACTORS

        // for chunk in chunks_of_symbols.clone() {
        //     let chunk = chunk.to_vec();
        //     let _result = actor_address.send(QuoteRequest { chunk, from, to }).await;
        // }

        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| async {
        //         actor_address
        //             .send(QuoteRequestMsg {
        //                 chunk: chunk.to_vec(),
        //                 from,
        //                 to,
        //             })
        //             .await
        //     })
        //     .collect();
        // let _ = futures::future::join_all(queries).await;

        // OLD WITHOUT ACTORS

        // // THE FASTEST SOLUTION - 1.2 s with chunk size of 5
        // // Explicit concurrency with async/await paradigm:
        // // Run multiple instances of the same Future concurrently.
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| handle_symbol_data(chunk, from, to))
        //     .collect();
        // let _ = futures::future::join_all(queries).await; // Vec<()>

        // // THE FASTEST SOLUTION - 1.2 s with chunk size of 5
        // // The `main()` function requires `#[actix::main]`.
        // // If we instead put `#[tokio::main]` it throws a panic.
        // let mut handles = vec![];
        // for chunk in chunks_of_symbols.clone() {
        //     let handle = tokio::spawn(handle_symbol_data(chunk, from, to));
        //     handles.push(handle);
        // }
        // let _ = futures::future::join_all(handles).await;

        println!("\nTook {:.3?} to complete.", start.elapsed());
    }

    System::current().stop();

    Ok(())
}
