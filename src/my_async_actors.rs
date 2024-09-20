//! My own asynchronous actor framework implementation
//!
//! Requires `#[tokio::main]`.

#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

use anyhow::{Context, Result};
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use yahoo_finance_api as yahoo;

use crate::async_signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};
use crate::constants::{
    ACTOR_CHANNEL_CAPACITY, CSV_FILE_PATH, CSV_HEADER, TAIL_BUFFER_SIZE, WINDOW_SIZE,
};
use crate::types::{
    Batch, CollectionMsgErrorType, MsgResponseType, TailResponse, UniversalMsgErrorType,
    WriterMsgErrorType,
};

// ============================================================================
//
//
//
//
//                     Traits [`Actor`] & [`ActorHandle`]
//
//
//
//
// ============================================================================

/// The [`Actor`] trait describes the actor methods.
///
/// It was not made public on purpose.
///
/// An actor instance can only be created through the [`ActorHandle`]
/// trait, which is public.
///
/// The type [`Self::Msg`] represents an incoming message type.
///
/// The type [`R`] represents a response message type.
///
/// Most errors are handled inside methods - they are not propagated.
///
/// The [`tokio::sync::mpsc::error::SendError`] error type isn't
/// informative; it doesn't return any useful piece of information.
/// It returns something like `SendError{..}`, which is not very
/// useful, and that's why we have decided to handle errors in the
/// methods and provide our own custom messages.
///
/// We are keeping the message response type as example.
/// In our use-case, in our custom solution, it is `()`.
/// We gave it a name, an alias: `MsgResponseType`.
/// It is used on some methods.
///
/// It could be a different type in general case, and since the trait
/// is generic, different Actor implementations can have different
/// message response types.
/// We are keeping it to make the solution a little more general, so
/// that it can be modified easily if needed.
///
/// Mind you, messages should **NOT** have a return value, as that
/// would mean that the sending actor needs to block while waiting
/// for the response.
/// The receiving actor can deliver results/response in a reply message.
///
/// Regardless, we kept it, as it can be used if we want to return
/// a result to the main thread, but we don't have to use it for
/// communication between actors.
///
/// Likewise, the trait has an associated type for messages, `Msg`,
/// to make it more general. It could have also been a generic type,
/// `M`, but it's a little better to have an associated type instead.
trait Actor<R> {
    /// The associated type [`Self::Msg`] represents an incoming message type.
    type Msg;

    /// Create a new [`Actor`]
    fn new(receiver: mpsc::Receiver<Self::Msg>) -> Self;

    /// Start the [`Actor`]
    async fn start(&mut self) -> Result<MsgResponseType> {
        #[cfg(debug_assertions)]
        tracing::debug!("Actor {:p} is started.", self);

        Ok(())
    }

    /// Run the [`Actor`]
    async fn run(&mut self) -> Result<R>;

    /// Stop the [`Actor`]
    fn stop(&mut self) {
        #[cfg(debug_assertions)]
        tracing::debug!("Actor {:p} is stopping.", self);
    }

    /// Handle the message
    async fn handle(&mut self, msg: Self::Msg) -> Result<R>;
}

/// The [`ActorHandle`] controls creation and execution of actors.
///
/// It can be used to create multiple instances of multiple
/// actor types.
///
/// Each handler creates a single actor instance.
///
/// Actors themselves were not made public on purpose.
///
/// We want to create them through actor handlers.
///
/// The type [`Self::Msg`] represents an incoming message type.
///
/// The type [`R`] represents a response message type.
///
/// The type [`E`] represents an error type.
pub(crate) trait ActorHandle<R, E> {
    /// The type [`Self::Msg`] represents an incoming message type.
    type Msg;

    /// Create a new [`ActorHandle`]
    ///
    /// This function creates a single [`Actor`] instance,
    /// and a MPSC channel for communicating to the actor.
    ///
    /// # Panics
    ///
    /// Panics if it can't run the actor.
    fn new() -> Self;

    /// Send a message to an [`Actor`] instance through the [`ActorHandle`]
    async fn send(&self, msg: Self::Msg) -> Result<R, E>;
}

// ============================================================================
//
//
//
//
//        [`ActorMessage`], [`UniversalActor`], [`UniversalActorHandle`]
//
//
//
//
// ============================================================================

/// The [`ActorMessage`] enumeration
///
/// Supports two message types:
/// - [`QuoteRequestsMsg`],
/// - [`SymbolsClosesMsg`],
///
/// There is no expected response for any of the message types.
///
/// We could have an oneshot channel for sending the response back in general case.
/// It could be used for every message type.
/// We simply don't need it in our specific (custom) case.
pub enum ActorMessage {
    QuoteRequestsMsg {
        symbols: Vec<String>,
        from: OffsetDateTime,
        to: OffsetDateTime,
        writer_handle: WriterActorHandle,
        collection_handle: CollectionActorHandle,
        start: Instant,
    },
    SymbolsClosesMsg {
        symbols_closes: HashMap<String, Vec<f64>>,
        from: OffsetDateTime,
        writer_handle: WriterActorHandle,
        collection_handle: CollectionActorHandle,
        start: Instant,
    },
}

/// A universal (general) type of actor
///
/// It can receive and handle two message types.
///
/// It was not made public on purpose.
///
/// It can only be created through [`UniversalActorHandle`], which is public.
struct UniversalActor {
    receiver: mpsc::Receiver<ActorMessage>,
}

impl Actor<MsgResponseType> for UniversalActor {
    type Msg = ActorMessage;

    /// Create a new [`UniversalActor`]
    fn new(receiver: mpsc::Receiver<ActorMessage>) -> Self {
        Self { receiver }
    }

    /// Run the [`UniversalActor`]
    async fn run(&mut self) -> Result<MsgResponseType> {
        tracing::debug!("UniversalActor {:p} is running.", self);

        while let Some(msg) = self.receiver.recv().await {
            self.handle(msg).await?;
        }

        Ok(())
    }

    /// Handle the [`ActorMessage`]
    async fn handle(&mut self, msg: ActorMessage) -> Result<MsgResponseType> {
        match msg {
            ActorMessage::QuoteRequestsMsg {
                symbols,
                from,
                to,
                writer_handle,
                collection_handle,
                start,
            } => {
                Self::handle_quote_requests_msg(
                    symbols,
                    from,
                    to,
                    writer_handle,
                    collection_handle,
                    start,
                )
                .await
                .expect("Expected some result from `handle_quote_requests_msg()`");
            }
            ActorMessage::SymbolsClosesMsg {
                symbols_closes,
                from,
                writer_handle,
                collection_handle,
                start,
            } => {
                Self::handle_symbols_closes_msg(
                    symbols_closes,
                    from,
                    writer_handle,
                    collection_handle,
                    start,
                )
                .await;
            }
        }

        Ok(())
    }
}

impl UniversalActor {
    /// The [`QuoteRequestsMsg`] message handler for the fetch [`UniversalActor`] actor
    ///
    /// Spawns a new processor [`UniversalActor`] and sends it a [`SymbolsClosesMsg`] message.
    ///
    /// The message contains a hash map of `symbols` and associated `Vec<f64>` with closing prices for that symbol
    /// in case there was no error when fetching the data, or an empty vector in case of an error,
    /// in which case it logs the error message at the warning level.
    ///
    /// So, in case of an API error for a symbol, when trying to fetch its data,
    /// we don't break the program but rather continue, skipping the symbol.
    ///
    /// # Errors
    /// - [yahoo_finance_api::YahooError](https://docs.rs/yahoo_finance_api/2.2.1/yahoo_finance_api/enum.YahooError.html)
    async fn handle_quote_requests_msg(
        symbols: Vec<String>,
        from: OffsetDateTime,
        to: OffsetDateTime,
        writer_handle: WriterActorHandle,
        collection_handle: CollectionActorHandle,
        start: Instant,
    ) -> Result<MsgResponseType> {
        let provider = yahoo::YahooConnector::new().context(format!("Skipping: {:?}", symbols))?;

        let mut symbols_closes: HashMap<String, Vec<f64>> = HashMap::with_capacity(symbols.len());

        for symbol in symbols {
            let closes = match Self::fetch_closing_data(&symbol, from, to, &provider).await {
                Ok(closes) => closes,
                Err(err) => {
                    tracing::warn!(
                        "There was an API error \"{}\" while fetching data for the symbol \"{}\"; \
                         skipping the symbol.",
                        err,
                        symbol
                    );
                    vec![]
                }
            };

            symbols_closes.insert(symbol, closes);
        }

        let symbols_closes_msg = ActorMessage::SymbolsClosesMsg {
            symbols_closes,
            from,
            writer_handle,
            collection_handle,
            start,
        };

        // Spawn another Actor and send it the message.
        let actor_handle = UniversalActorHandle::new();
        actor_handle
            .send(symbols_closes_msg)
            .await
            .context("Couldn't send a message to the ProcessorActor.")?;

        Ok(())
    }

    /// The [`SymbolsClosesMsg`] message handler for the processor [`UniversalActor`] actor
    ///
    /// Sends a [`PerformanceIndicatorsRowsMsg`] message to the [`WriterActor`],
    /// whose address it gets from the [`SymbolsClosesMsg`] message.
    async fn handle_symbols_closes_msg(
        symbols_closes: HashMap<String, Vec<f64>>,
        from: OffsetDateTime,
        writer_handle: WriterActorHandle,
        collection_handle: CollectionActorHandle,
        start: Instant,
    ) -> MsgResponseType {
        let from = OffsetDateTime::format(from, &Rfc3339).expect("Couldn't format 'from'.");

        let mut rows: Vec<PerformanceIndicatorsRow> = Vec::with_capacity(symbols_closes.len());

        for symbol_closes in symbols_closes {
            let symbol = symbol_closes.0;
            let closes = symbol_closes.1;

            if !closes.is_empty() {
                let min = MinPrice {};
                let max = MaxPrice {};
                let price_diff = PriceDifference {};
                let n_window_sma = WindowedSMA {
                    window_size: WINDOW_SIZE,
                };

                let last_price = *closes.last().expect("Expected non-empty closes.");
                let (_, pct_change) = price_diff.calculate(&closes).await.unwrap_or((0., 0.));
                let pct_change = pct_change * 100.0;
                let period_min: f64 = min.calculate(&closes).await.unwrap_or_default();
                let period_max: f64 = max.calculate(&closes).await.unwrap_or_default();
                let sma = n_window_sma.calculate(&closes).await.unwrap_or(vec![]);
                let sma = *sma.last().unwrap_or(&0.0);

                let row = PerformanceIndicatorsRow {
                    symbol: symbol.clone(),
                    last_price,
                    pct_change,
                    period_min,
                    period_max,
                    sma,
                };

                rows.push(row);

                // A simple way to output CSV data
                tracing::info!(
                    "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
                    from,
                    symbol,
                    last_price,
                    pct_change,
                    period_min,
                    period_max,
                    sma,
                );
            } else {
                tracing::warn!("Got no data for symbol \"{}\".", symbol);
            }
        }

        // Assemble a message for the single writer actor.
        let perf_ind_msg = PerformanceIndicatorsRowsMsg { from, rows, start };

        // Send the message to the single writer actor.
        writer_handle
            .send(perf_ind_msg.clone())
            .await
            .expect("Couldn't send a message to the WriterActor.");

        // Assemble a message for the single collection actor.
        let coll_msg = CollectionActorMsg::PerformanceIndicatorsChunk(perf_ind_msg);
        // let coll_msg = CollectionActorMsg::PerformanceIndicatorsChunk {
        //     from: perf_ind_msg.from,
        //     rows: perf_ind_msg.rows,
        //     start,
        // };

        // Send the message to the single collection actor.
        collection_handle
            .send(coll_msg)
            .await
            .expect("Couldn't send a message to the CollectionActor.");
    }

    /// Retrieve data for a single `symbol` from a data source (`provider`) and extract the closing prices
    ///
    /// # Returns
    /// - Vector of closing prices in case of no error, or,
    ///
    /// # Errors
    /// - [`yahoo::YahooError`](https://docs.rs/yahoo_finance_api/2.2.1/yahoo_finance_api/enum.YahooError.html)
    ///   in case of an error.
    async fn fetch_closing_data(
        symbol: &str,
        from: OffsetDateTime,
        to: OffsetDateTime,
        provider: &yahoo::YahooConnector,
    ) -> Result<Vec<f64>, yahoo::YahooError> {
        // This function takes a single symbol.
        // The crate that we're using doesn't contain a function that works with a chunk of symbols.
        let yresponse = provider.get_quote_history(symbol, from, to).await?;

        let mut quotes = yresponse.quotes()?;

        let mut result = vec![];
        if !quotes.is_empty() {
            quotes.sort_by_cached_key(|k| k.timestamp);
            result = quotes.iter().map(|q| q.adjclose).collect();
        }

        Ok(result)
    }
}

impl Drop for UniversalActor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A handle for the [`UniversalActor`]
///
/// Only the handle is public; the [`UniversalActor`] isn't.
///
/// We can only create [`UniversalActor`]s through the [`UniversalActorHandle`].
///
/// It contains the `sender` field, which represents
/// a sender of the [`ActorMessage`] in an MPSC channel.
///
/// The handle is the sender, and the actor is the receiver
/// of a message in the channel.
///
/// We only create a single [`UniversalActor`] instance in an [`UniversalActorHandle`].
#[derive(Clone)]
pub struct UniversalActorHandle {
    sender: mpsc::Sender<ActorMessage>,
}

impl ActorHandle<MsgResponseType, UniversalMsgErrorType> for UniversalActorHandle {
    type Msg = ActorMessage;

    /// Create a new [`UniversalActorHandle`]
    ///
    /// This function creates a single [`UniversalActor`] instance,
    /// and a MPSC channel for communicating to the actor.
    ///
    /// # Panics
    ///
    /// Panics if it can't run the actor.
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel(ACTOR_CHANNEL_CAPACITY);
        let mut actor = UniversalActor::new(receiver);
        tokio::spawn(async move { actor.run().await });

        Self { sender }
    }

    /// Send a message to an [`UniversalActor`] instance through the [`UniversalActorHandle`]
    async fn send(&self, msg: ActorMessage) -> Result<MsgResponseType, UniversalMsgErrorType> {
        self.sender.send(msg).await
    }
}

// ============================================================================
//
//
//
//
//  [`PerformanceIndicatorsRowsMsg`], [`WriterActor`], [`WriterActorHandle`]
//
//
//
//
// ============================================================================

/// A single row of calculated performance indicators for a symbol
#[derive(Clone, Debug, Serialize)]
pub struct PerformanceIndicatorsRow {
    pub symbol: String,
    pub last_price: f64,
    pub pct_change: f64,
    pub period_min: f64,
    pub period_max: f64,
    pub sma: f64,
}

/// The [`PerformanceIndicatorsRowsMsg`] message
///
/// It contains a `from` date and time field,
/// and calculated performance indicators for a **chunk** of symbols.
///
/// There is no expected response.
///
/// We could have an oneshot channel for sending the response back in general case.
/// We simply don't need it in our specific (custom) case.
#[derive(Clone)]
pub struct PerformanceIndicatorsRowsMsg {
    from: String,
    rows: Vec<PerformanceIndicatorsRow>,
    start: Instant,
}

/// Actor for writing calculated performance indicators for fetched stock data into a CSV file
///
/// It is not made public on purpose.
///
/// It can only be created through [`WriterActorHandle`], which is public.
struct WriterActor {
    receiver: mpsc::Receiver<PerformanceIndicatorsRowsMsg>,
    pub file_name: String,
    pub writer: Option<BufWriter<File>>,
}

impl Actor<MsgResponseType> for WriterActor {
    type Msg = PerformanceIndicatorsRowsMsg;

    /// Create a new [`WriterActor`]
    fn new(receiver: mpsc::Receiver<PerformanceIndicatorsRowsMsg>) -> Self {
        Self {
            receiver,
            file_name: CSV_FILE_PATH.to_string(),
            // file_name: OffsetDateTime::now_utc()
            //     .format(&Rfc3339) // or Rfc2822 (has blanks), Iso8601
            //     .expect("The provided date or time format isn't correct."),
            writer: None,
        }
    }

    /// Start the [`WriterActor`]
    ///
    /// This function is meant to be used directly in the [`WriterActorHandle`].
    async fn start(&mut self) -> Result<MsgResponseType> {
        let mut file = File::create(&self.file_name)
            .unwrap_or_else(|_| panic!("Could not open target file \"{}\".", self.file_name));
        #[cfg(debug_assertions)]
        tracing::debug!("The output file path is \"{}\".", self.file_name);
        let _ = writeln!(&mut file, "{}", CSV_HEADER);
        self.writer = Some(BufWriter::new(file));
        tracing::debug!("WriterActor is started.");

        self.run().await?;

        Ok(())
    }

    /// Run the [`WriterActor`]
    ///
    /// This function is meant to be used indirectly - only through the [`WriterActor::start`] function
    async fn run(&mut self) -> Result<MsgResponseType> {
        tracing::debug!("WriterActor is running.");

        while let Some(msg) = self.receiver.recv().await {
            self.handle(msg).await?;
        }

        Ok(())
    }

    /// Stop the [`WriterActor`]
    ///
    /// This function is meant to be called in the [`WriterActor`]'s destructor.
    fn stop(&mut self) {
        if let Some(writer) = &mut self.writer {
            writer
                .flush()
                .expect("Failed to flush writer. Data loss :(")
        };

        tracing::debug!("WriterActor is flushed and properly stopped.");
    }

    /// The [`PerformanceIndicatorsRowsMsg`] message handler for the [`WriterActor`] actor
    ///
    /// Writes results to file and measures & prints the iteration's execution time.
    async fn handle(&mut self, msg: PerformanceIndicatorsRowsMsg) -> Result<MsgResponseType> {
        let from = msg.from;
        let rows = msg.rows;
        let start = msg.start;

        if let Some(file) = &mut self.writer {
            for row in rows {
                let _ = writeln!(
                    file,
                    "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
                    from,
                    row.symbol,
                    row.last_price,
                    row.pct_change,
                    row.period_min,
                    row.period_max,
                    row.sma,
                );
            }

            file.flush()
                .context("Failed to flush to file. Data loss :/")?;
        }

        tracing::info!("Took {:.3?} to complete.", start.elapsed());
        #[cfg(debug_assertions)]
        println!("Took {:.3?} to complete.\n", start.elapsed());

        Ok(())
    }
}

impl Drop for WriterActor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A handle for the [`WriterActor`]
///
/// Only the handle is public; the [`WriterActor`] isn't.
///
/// We can only create [`WriterActor`]s through the [`WriterActorHandle`].
///
/// It contains the `sender` field, which represents
/// a sender of the [`PerformanceIndicatorsRowsMsg`] in an MPSC channel.
///
/// The handle is the sender, and the actor is the receiver
/// of a message in the channel.
///
/// We only create a single [`WriterActor`] instance in a [`WriterActorHandle`].
#[derive(Clone)]
pub struct WriterActorHandle {
    sender: mpsc::Sender<PerformanceIndicatorsRowsMsg>,
}

impl ActorHandle<MsgResponseType, WriterMsgErrorType> for WriterActorHandle {
    type Msg = PerformanceIndicatorsRowsMsg;

    /// Create a new [`WriterActorHandle`]
    ///
    /// This function creates a single [`WriterActor`] instance,
    /// and a MPSC channel for communicating with the actor.
    ///
    /// It also starts (runs) the actor.
    ///
    /// # Panics
    ///
    /// Panics if it can't run the actor.
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel(ACTOR_CHANNEL_CAPACITY);
        let mut actor = WriterActor::new(receiver);
        tokio::spawn(async move { actor.start().await });

        Self { sender }
    }

    /// Send a message to an [`WriterActor`] instance through the [`WriterActorHandle`]
    async fn send(
        &self,
        msg: PerformanceIndicatorsRowsMsg,
    ) -> Result<MsgResponseType, WriterMsgErrorType> {
        self.sender.send(msg).await
    }
}

// ============================================================================
//
//
//
//
//    [`CollectionActorMsg`], [`CollectionActor`], [`CollectionActorHandle`],
//                        [`Batch`], [`TailResponse`]
//
//
//
//
// ============================================================================

/// The [`CollectionActorMsg`] enumeration
///
/// Supports two message types:
/// - [`TailRequest`],
/// - [`PerformanceIndicatorsChunk`],
///
/// There is no expected response for any of the message types.
///
/// We could have an oneshot channel for sending the response back in general case.
/// It could be used for every message type.
/// We simply don't need it in our specific (custom) case.
pub enum CollectionActorMsg {
    /// Wraps a [`PerformanceIndicatorsRowsMsg`] message
    PerformanceIndicatorsChunk(PerformanceIndicatorsRowsMsg),
    /// A request from web server for the last `n` batches of processed data
    TailRequest {
        sender: mpsc::Sender<TailResponse>,
        n: usize,
    },
}

// TODO: update docstring
/// Actor for collecting calculated performance indicators for fetched stock data into a buffer
///
/// It is used for storing the performance data in a buffer of capacity `N`,
/// where `N` is the number of main loop iterations that occur at a fixed time interval.
///
/// These data can then be fetched by the web server.
///
/// It is not made public on purpose.
///
/// It can only be created through [`CollectionActorHandle`], which is public.
struct CollectionActor {
    receiver: mpsc::Receiver<CollectionActorMsg>,
    buffer: TailResponse,
    batch: Batch,
    cnt: usize,
}

impl Actor<MsgResponseType> for CollectionActor {
    type Msg = CollectionActorMsg;

    /// Create a new [`CollectionActor`]
    fn new(receiver: mpsc::Receiver<CollectionActorMsg>) -> Self {
        Self {
            receiver,
            // buffer: VecDeque::with_capacity(TAIL_BUFFER_SIZE), // todo
            buffer: Vec::with_capacity(TAIL_BUFFER_SIZE),
            batch: Vec::with_capacity(505), // todo
            cnt: 0,
        }
    }

    /// Start the [`CollectionActor`]
    ///
    /// This function is meant to be used directly in the [`CollectionActorHandle`].
    async fn start(&mut self) -> Result<MsgResponseType> {
        #[cfg(debug_assertions)]
        tracing::debug!("CollectionActor is starting...");

        tracing::debug!("CollectionActor is started.");

        self.run().await?;

        Ok(())
    }

    /// Run the [`CollectionActor`]
    ///
    /// This function is meant to be used indirectly - only through the [`CollectionActor::start`] function
    async fn run(&mut self) -> Result<MsgResponseType> {
        tracing::debug!("CollectionActor is running.");

        while let Some(msg) = self.receiver.recv().await {
            self.handle(msg).await?;
        }

        Ok(())
    }

    /// Stop the [`CollectionActor`]
    ///
    /// This function is meant to be called in the [`CollectionActor`]'s destructor.
    fn stop(&mut self) {
        tracing::debug!("CollectionActor is stopped.");
    }

    /// The [`CollectionActorMsg`] message handler for the [`CollectionActor`] actor
    ///
    /// It collects chunks from Processing [`UniversalActor`]s, assembles them in batches,
    /// and then stores the batches in the buffer.
    ///
    /// It also receives requests from web server for the last n batches of
    /// processed data.
    async fn handle(&mut self, msg: CollectionActorMsg) -> Result<MsgResponseType> {
        match msg {
            CollectionActorMsg::PerformanceIndicatorsChunk(msg) => {
                Self::handle_perf_ind_chunk(self, msg).await;
                // .expect("Expected some result from `handle_perf_ind_chunk()`"); // todo
            }
            CollectionActorMsg::TailRequest { sender, n } => {
                Self::handle_tail_request(self, sender, n).await?;
            }
        }

        Ok(())
    }
}

impl CollectionActor {
    /// Handle a [`CollectionActorMsg::PerformanceIndicatorsChunk`] message,
    /// which wraps a [`PerformanceIndicatorsRowsMsg`] message
    ///
    /// Assembles chunks into complete batches and stores them in buffer.
    ///
    /// This ensures that a batch cannot be partially read by the web server.
    /// It can only be fully-read, when it's ready.
    ///
    /// This message comes from a processing actor.
    async fn handle_perf_ind_chunk(
        &mut self,
        msg: PerformanceIndicatorsRowsMsg,
        // ) -> Result<MsgResponseType> {
    ) -> MsgResponseType {
        let rows = msg.rows;

        // TODO: When all chunks have been received, assemble a new batch from them and store it in the buffer.
        self.cnt += 1;
        self.batch.extend(rows);

        // todo: this is not fixed to 2 or to 101!
        // todo: also, cloning is not efficient; we could use a flag to mark it when the batch is ready for reading (when it's fully-assembled)
        if self.cnt == 2 {
            // self.buffer.push_back(self.batch.clone());
            self.buffer.push(self.batch.clone());
            self.cnt = 0;
        }
        // self.buffer.extend(rows); remove

        // Ok(())
    }

    /// Handle a [`CollectionActorMsg::TailRequest`]
    ///
    /// Gets the last fully-assembled `n` batches of performance indicators,
    /// and sends them to the web server.
    ///
    /// Takes care of keeping only the fresh data in the buffer, so that its
    /// size doesn't ever grow, which prevents memory leaks.
    /// Old data are removed from the buffer to make room for new data.
    ///
    /// This message comes from the web server.
    async fn handle_tail_request(
        &mut self,
        sender: mpsc::Sender<TailResponse>,
        n: usize,
    ) -> Result<MsgResponseType> {
        // todo: do this modulo capacity
        let response = self.buffer.iter().take(n).cloned().collect();
        sender.send(response).await.unwrap();

        // let response = &self.buffer[..n];
        // let response = &self.buffer;
        // sender
        //     .send(<TailResponse>::try_from(response).unwrap())
        //     .await
        //     .unwrap();
        // .context("Failed to send response")?;

        // sender.send(*response).await.unwrap();
        // .context("Failed to send response")?;

        Ok(())
    }
}

impl Drop for CollectionActor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A handle for the [`CollectionActor`]
///
/// Only the handle is public; the [`CollectionActor`] isn't.
///
/// We can only create [`CollectionActor`]s through the [`CollectionActorHandle`].
///
/// It contains the `sender` field, which represents
/// a sender of the [`CollectionActorMsg`] in an MPSC channel.
///
/// The handle is the sender, and the actor is the receiver
/// of a message in the channel.
///
/// We only create a single [`CollectionActor`] instance in a [`CollectionActorHandle`].
#[derive(Clone)]
pub struct CollectionActorHandle {
    sender: mpsc::Sender<CollectionActorMsg>,
}

impl ActorHandle<MsgResponseType, CollectionMsgErrorType> for CollectionActorHandle {
    type Msg = CollectionActorMsg;

    /// Create a new [`CollectionActorHandle`]
    ///
    /// This function creates a single [`CollectionActor`] instance,
    /// and a MPSC channel for communicating with the actor.
    ///
    /// It also starts (runs) the actor.
    ///
    /// # Panics
    ///
    /// Panics if it can't run the actor.
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel(ACTOR_CHANNEL_CAPACITY);
        let mut actor = CollectionActor::new(receiver);
        tokio::spawn(async move { actor.start().await });

        Self { sender }
    }

    /// Send a message to an [`CollectionActor`] instance through the [`CollectionActorHandle`]
    async fn send(
        &self,
        msg: CollectionActorMsg,
    ) -> Result<MsgResponseType, CollectionMsgErrorType> {
        self.sender.send(msg).await
    }
}
