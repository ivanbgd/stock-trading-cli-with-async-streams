use std::sync::OnceLock;
use std::time::{Duration, Instant};

// use actix::{Actor, SyncArbiter};
// use actix_rt::System;
// use async_std::stream::{self, StreamExt};
use async_std::stream::{self, StreamExt};
use clap::Parser;
// use rayon::prelude::*;
// use rayon::prelude::*;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

// use crate::actix_async_actors::{handle_symbol_data, WriterActor};
use crate::cli::Args;
use crate::constants::{CHUNK_SIZE, CSV_HEADER, TICK_INTERVAL_SECS};
use crate::my_async_actors::{ActorHandle, ActorMessage, UniversalActorHandle, WriterActorHandle};

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
// pub async fn main_loop() -> Result<MsgResponseType, actix::MailboxError> {
pub async fn main_loop() {
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
    let chunks_of_symbols: Vec<&[String]> = symbols.chunks(CHUNK_SIZE).collect();
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

    // // More than one thread is certainly incorrect, but even with only one thread
    // // not everything gets written to the file, so we can't consider that correct either.
    // // Not all symbols are written, but even some rows are not complete.
    // // In fact, not all symbols get printed to stdout, but more do than to the file.
    // // With stdout at least all rows are complete, but they are printed in a different actor.
    // // This solution is asynchronous (async/await), so that could be the culprit.
    // let writer_address = SyncArbiter::start(1, || WriterActor {
    //     file_name: "output.csv".to_string(),
    //     writer: None,
    // });

    // // We need to ensure that we have one and only one `WriterActor` - a singleton.
    // // This is because it writes to a file, and writing to a shared object,
    // // such as a file, needs to be synchronized, i.e., sequential.
    // // We generally don't use low-level synchronization primitives such as
    // // locks, mutexes, and similar when working with Actors.
    // // Actors have mailboxes and process messages that they receive one at a time,
    // // i.e., sequentially, and hence we can accomplish synchronization implicitly
    // // by using a single writer actor.
    // let writer_address = WriterActor::new().start();

    let writer_handle = WriterActorHandle::new();

    let mut interval = stream::interval(Duration::from_secs(TICK_INTERVAL_SECS));

    while let Some(_) = interval.next().await {
        //     TODO: uncomment
        //     todo: remove the FOR line
        // for _ in 0..1 {
        // We always want a fresh period end time, which is "now" in the UTC time zone.
        let to = OffsetDateTime::now_utc();

        // For standard output only, i.e., not for CSV
        println!("\n\n*** {} ***\n", to);

        // A simple way to output a CSV header
        println!("{}", CSV_HEADER);

        let start = Instant::now();

        // NEW WITH MY OWN IMPLEMENTATION OF ACTORS

        // Without rayon. Not sequential. Multiple "`FetchActor`s" and "`ProcessorActor`s".
        // This is fast! Possibly even below a second.

        // We start multiple instances of `Actor` - one per chunk of symbols,
        // and they will start the next `Actor` in the process - one each.
        // A single `ActorHandle` creates a single `Actor` instance and runs it on a new Tokio (asynchronous) task.
        //
        // Explicit concurrency with async/await paradigm: Run multiple instances of the same Future concurrently.
        // That's why it's fast - we spawn multiple tasks, i.e., multiple actors, concurrently, at the same time.
        // They'll also spawn multiple "`ProcessorActor`s" concurrently (at the same time).
        for chunk in chunks_of_symbols.clone() {
            let actor_handle = UniversalActorHandle::new();
            let _ = actor_handle
                .send(ActorMessage::QuoteRequestsMsg {
                    symbols: chunk.into(),
                    from,
                    to,
                    writer_handle: writer_handle.clone(), // not cloneable!
                })
                .await;
        }

        // // With rayon. Same speed as without rayon; fast (chunks or par_chunks don't make a difference).
        //
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| async {
        //         ActorHandle::new()
        //             .send(ActorMessage::QuoteRequestsMsg {
        //                 symbols: (*chunk).into(),
        //                 from,
        //                 to,
        //                 // writer_address: writer_address.clone(),
        //             })
        //             .await
        //     })
        //     .collect();
        // let _ = futures::future::join_all(queries).await;

        // NEW WITH ACTORS

        // Without rayon. Not sequential. Multiple `FetchActor`s and `ProcessorActor`s. Possibly around 1.5 seconds.

        // // We start multiple `FetchActor`s - one per chunk of symbols,
        // // and they will start the next Actor in the process - one each.
        // // Explicit concurrency with async/await paradigm: Run multiple instances of the same Future concurrently.
        // // That's why it's fast - we spawn multiple tasks, i.e., multiple actors, concurrently, at the same time.
        // // They'll also spawn multiple `ProcessorActor`s concurrently (at the same time).
        // for chunk in chunks_of_symbols.clone() {
        //     let fetch_address = FetchActor.start();
        //
        //     let _ = fetch_address
        //         .send(QuoteRequestsMsg {
        //             chunk: chunk.into(),
        //             from,
        //             to,
        //             writer_address: writer_address.clone(),
        //         })
        //         .await?;
        // }

        // // With rayon. Not sequential. Multiple `FetchActor`s and `ProcessorActor`s. Possibly around 1.5 seconds.
        // // It is not much faster (if at all) than the above solution without rayon.
        // // Namely, execution time is not measured properly in this case, but it's roughly the same.
        // // Performance is the same when using regular (core) `chunks()` and `rayon`'s `par_chunks()`.
        //
        // // We start multiple `FetchActor`s - one per chunk of symbols,
        // // and they will start the next Actor in the process - one each.
        // // Explicit concurrency with async/await paradigm: Run multiple instances of the same Future concurrently.
        // // That's why it's fast - we spawn multiple tasks, i.e., multiple actors, concurrently, at the same time.
        // // They'll also spawn multiple `ProcessorActor`s concurrently (at the same time).
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| async {
        //         FetchActor
        //             .start()
        //             .send(QuoteRequestsMsg {
        //                 chunk: (*chunk).into(),
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

    // System::current().stop();

    // Ok(())
}
