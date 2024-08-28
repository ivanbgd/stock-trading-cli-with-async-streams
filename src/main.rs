use anyhow::Result;
use axum::Router;
use axum::routing::get;
use tracing_subscriber::EnvFilter;

use stock::constants::{ADDRESS, SHUTDOWN_INTERVAL_SECS};
use stock::handlers::{get_desc, get_tail, root};
use stock::logic::main_loop;
use stock::types::MsgResponseType;
use stock_trading_cli_with_async_streams as stock;

// #[actix::main]
#[tokio::main]
async fn main() -> Result<MsgResponseType> {
    // initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("starting the app");

    // Actix and Tokio have an impedance mismatch because Actix futures are !Send.
    // There's friction between the two runtimes.
    // When I tried to combine all six implementation variants, I got this error:
    // thread 'tokio-runtime-worker' panicked at /Users/ivan/.cargo/registry/src/index.crates.io-6f17d22bba15001f/actix-0.13.5/src/context.rs:148:9:
    // `spawn_local` called from outside of a `task::LocalSet`
    // For context:
    // https://github.com/actix/actix-web/issues/1283
    // https://docs.rs/tokio/latest/tokio/task/struct.LocalSet.html
    // https://docs.rs/actix-web/latest/actix_web/rt/index.html#running-actix-web-using-tokiomain
    // Now, this is an attempt to solve, but it doesn't fully work, as it doesn't
    // start our main loop or the axum server (depending on how we implement stuff).

    // spawn application as a separate task

    // With #[tokio::main]:
    // "thread 'tokio-runtime-worker' panicked at /Users/ivan/.cargo/registry/src/index.crates.io-6f17d22bba15001f/actix-0.13.5/src/context.rs:148:9:
    // `spawn_local` called from outside of a `task::LocalSet`"
    //
    // With #[actix::main]:
    // "thread 'main' panicked at /Users/ivan/.cargo/registry/src/index.crates.io-6f17d22bba15001f/actix-0.13.5/src/context.rs:148:9:
    // `spawn_local` called from outside of a `task::LocalSet`"
    tokio::spawn(async move { main_loop().await });

    // doesn't start main loop - doesn't panic, but simply ignores it
    // std::thread::spawn(|| async move { main_loop().await });

    // "thread 'main' panicked at /Users/ivan/.cargo/registry/src/index.crates.io-6f17d22bba15001f/actix-rt-2.10.0/src/system.rs:57:30:
    // Cannot start a runtime from within a runtime. This happens because a function (like `block_on`) attempted to block the current thread while the thread is being used to drive asynchronous tasks."
    //
    // We would need to remove `#[tokio::main]`, but we need it for `local.run_until()` below.
    // actix_rt::System::new()
    //     .block_on(async move { main_loop().await })
    //     .unwrap();

    // Don't wrap main() with any runtime!
    // "thread 'main-tokio' panicked at /Users/ivan/.cargo/registry/src/index.crates.io-6f17d22bba15001f/actix-0.13.5/src/context.rs:148:9:
    // `spawn_local` called from outside of a `task::LocalSet`"
    // actix_rt::System::with_tokio_rt(|| {
    //     tokio::runtime::Builder::new_multi_thread()
    //         .enable_all()
    //         .worker_threads(stock::constants::NUM_THREADS)
    //         .thread_name("main-tokio")
    //         .build()
    //         .unwrap()
    // })
    // .block_on(async_main());

    let local = tokio::task::LocalSet::new();

    // local
    //     .run_until(async move {
    //         // spawn application as a separate task
    //         let _ = tokio::task::spawn_local(async move { main_loop().await })
    //             .await
    //             .expect("Couldn't spawn-local");
    //
    //         // build our application with a route
    //         let app = Router::new()
    //             .route("/", get(root))
    //             .route("/desc", get(get_desc))
    //             .route("/tail/:n", get(get_tail));
    //
    //         // run our app with hyper
    //         let listener = tokio::net::TcpListener::bind(ADDRESS).await.unwrap();
    //         tracing::info!("listening on {}", listener.local_addr().unwrap());
    //         axum::serve(listener, app).await.unwrap();
    //
    //         // await the shutdown signal
    //         match tokio::signal::ctrl_c().await {
    //             Ok(()) => {
    //                 tracing::info!(
    //                     "\nCTRL+C received. Giving tasks some time ({} s) to finish...",
    //                     SHUTDOWN_INTERVAL_SECS
    //                 );
    //                 tokio::time::sleep(tokio::time::Duration::from_secs(SHUTDOWN_INTERVAL_SECS))
    //                     .await;
    //             }
    //             Err(err) => {
    //                 // We also shut down in case of an error.
    //                 tracing::error!("Unable to listen for the shutdown signal: {}", err);
    //             }
    //         }
    //     })
    //     .await;

    // "thread 'tokio-runtime-worker' panicked at /Users/ivan/.cargo/registry/src/index.crates.io-6f17d22bba15001f/actix-0.13.5/src/context.rs:148:9:
    // `spawn_local` called from outside of a `task::LocalSet`"
    local
        .run_until(async move {
            // build our application with a route
            let app = Router::new()
                .route("/", get(root))
                .route("/desc", get(get_desc))
                .route("/tail/:n", get(get_tail));

            // run our app with hyper
            let listener = tokio::net::TcpListener::bind(ADDRESS).await.unwrap();
            tracing::info!("listening on {}", listener.local_addr().unwrap());
            axum::serve(listener, app).await.unwrap();
        })
        .await;

    // await the shutdown signal
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!(
                "\nCTRL+C received. Giving tasks some time ({} s) to finish...",
                SHUTDOWN_INTERVAL_SECS
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(SHUTDOWN_INTERVAL_SECS)).await;
        }
        Err(err) => {
            // We also shut down in case of an error.
            tracing::error!("Unable to listen for the shutdown signal: {}", err);
        }
    }

    local.await;

    tracing::info!("Exiting now.");

    Ok(())
}

async fn _async_main() {
    tokio::spawn(async move {
        println!("From main tokio thread");
        tracing::info!("From main tokio thread");

        let _ = main_loop().await;

        // Would panic if uncommented printing: "System is not running"
        // println!("{:?}", actix_rt::System::current());
    });

    // build our application with a route
    let app = Router::new()
        .route("/", get(root))
        .route("/desc", get(get_desc))
        .route("/tail/:n", get(get_tail));

    // run our app with hyper
    let listener = tokio::net::TcpListener::bind(ADDRESS).await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    // await the shutdown signal
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!(
                "\nCTRL+C received. Giving tasks some time ({} s) to finish...",
                SHUTDOWN_INTERVAL_SECS
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(SHUTDOWN_INTERVAL_SECS)).await;
        }
        Err(err) => {
            // We also shut down in case of an error.
            tracing::error!("Unable to listen for the shutdown signal: {}", err);
        }
    }
}
