use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use yahoo_finance_api as yahoo;

use crate::async_signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};
use crate::constants::{CSV_FILE_NAME, CSV_HEADER, MPSC_CHANNEL_CAPACITY, WINDOW_SIZE};
use crate::types::{MsgResponseType, UniversalMsgErrorType, WriterMsgErrorType};

// ============================================================================
//
//                     Traits [`Actor`] & [`ActorHandle`]
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
/// All errors are handled inside methods - they are not propagated.
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
/// Likewise, the trait has an associated type for messages to make it
/// more general. It could have also been a generic type, `M`, but it's
/// a little better to have an associated type instead.
trait Actor<R> {
    /// The type [`Self::Msg`] represents an incoming message type.
    type Msg;

    /// Create a new [`Actor`]
    fn new(receiver: mpsc::Receiver<Self::Msg>) -> Self;

    /// Start the [`Actor`]
    async fn start(&mut self) {}

    /// Run the [`Actor`]
    async fn run(&mut self) -> R;

    /// Stop the [`Actor`]
    fn stop(&mut self) {}

    /// Handle the message
    async fn handle(&mut self, msg: Self::Msg) -> R;
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
//        [`ActorMessage`], [`UniversalActor`], [`UniversalActorHandle`]
//
// ============================================================================

/// The [`ActorMessage`] enumeration
///
/// Supports two message types:
/// - QuoteRequestsMsg,
/// - SymbolsClosesMsg,
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
    },
    SymbolsClosesMsg {
        symbols_closes: HashMap<String, Vec<f64>>,
        from: OffsetDateTime,
        writer_handle: WriterActorHandle,
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
    async fn run(&mut self) -> MsgResponseType {
        while let Some(msg) = self.receiver.recv().await {
            self.handle(msg).await;
        }
    }

    /// Handle the message
    async fn handle(&mut self, msg: ActorMessage) -> MsgResponseType {
        match msg {
            ActorMessage::QuoteRequestsMsg {
                symbols,
                from,
                to,
                writer_handle,
            } => {
                Self::handle_quote_requests_msg(symbols, from, to, writer_handle).await;
            }
            ActorMessage::SymbolsClosesMsg {
                symbols_closes,
                from,
                writer_handle,
            } => {
                Self::handle_symbols_closes_msg(symbols_closes, from, writer_handle).await;
            }
        }
    }
}

impl UniversalActor {
    /// The [`QuoteRequestsMsg`] message handler for the fetch [`UniversalActor`] actor
    ///
    /// Spawns a new processor [`UniversalActor`] and sends it a [`SymbolsClosesMsg`] message.
    ///
    /// The message contains a hash map of `symbols` and associated `Vec<f64>` with closing prices for that symbol
    /// in case there was no error when fetching the data, or an empty vector in case of an error,
    /// in which case it prints the error message to `stderr`.
    ///
    /// So, in case of an API error for a symbol, when trying to fetch its data,
    /// we don't break the program but rather continue.
    async fn handle_quote_requests_msg(
        symbols: Vec<String>,
        from: OffsetDateTime,
        to: OffsetDateTime,
        writer_handle: WriterActorHandle,
    ) -> MsgResponseType {
        let provider = yahoo::YahooConnector::new();

        let mut symbols_closes: HashMap<String, Vec<f64>> = HashMap::with_capacity(symbols.len());

        for symbol in symbols {
            let closes = match Self::fetch_closing_data(&symbol, from, to, &provider).await {
                Ok(closes) => closes,
                Err(err) => {
                    println!(
                        "There was an API error \"{}\" while fetching data for the symbol \"{}\"; \
                         skipping the symbol.",
                        err, symbol
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
        };

        // Spawn another Actor and send it the message.
        let actor_handle = UniversalActorHandle::new();
        actor_handle
            .send(symbols_closes_msg)
            .await
            .expect("Couldn't send a message to the ProcessorActor.");
    }

    /// The [`SymbolsClosesMsg`] message handler for the processor [`UniversalActor`] actor
    ///
    /// Sends a [`PerformanceIndicatorsRowsMsg`] message to the [`WriterActor`],
    /// whose address it gets from the [`SymbolsClosesMsg`] message.
    async fn handle_symbols_closes_msg(
        symbols_closes: HashMap<String, Vec<f64>>,
        from: OffsetDateTime,
        writer_handle: WriterActorHandle,
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
                println!(
                    "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
                    from, symbol, last_price, pct_change, period_min, period_max, sma,
                );
            } else {
                eprintln!("Got no data for the symbol \"{}\".", symbol);
            }
        }

        let perf_ind_msg = PerformanceIndicatorsRowsMsg { from, rows };

        // Send the message to the single writer actor.
        writer_handle
            .send(perf_ind_msg)
            .await
            .expect("Couldn't send a message to the WriterActor.");
    }

    /// Retrieve data for a single `symbol` from a data source (`provider`) and extract the closing prices
    ///
    /// # Returns
    /// - Vector of closing prices in case of no error, or,
    /// - [`yahoo::YahooError`](https://docs.rs/yahoo_finance_api/2.1.0/yahoo_finance_api/enum.YahooError.html)
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
        let (sender, receiver) = mpsc::channel(MPSC_CHANNEL_CAPACITY);
        let mut actor = UniversalActor::new(receiver);
        tokio::spawn(async move { actor.run().await });

        Self { sender }
    }

    /// Send a message to an [`UniversalActor`] instance through the [`UniversalActorHandle`]
    async fn send(&self, msg: ActorMessage) -> Result<MsgResponseType, UniversalMsgErrorType> {
        Ok(self.sender.send(msg).await?)
    }
}

// ============================================================================
//
//  [`PerformanceIndicatorsRowsMsg`], [`WriterActor`], [`WriterActorHandle`]
//
// ============================================================================

/// A single row of calculated performance indicators for a symbol
struct PerformanceIndicatorsRow {
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
/// and calculated performance indicators for a chunk of symbols.
///
/// There is no expected response.
///
/// We could have an oneshot channel for sending the response back in general case.
/// We simply don't need it in our specific (custom) case.
pub struct PerformanceIndicatorsRowsMsg {
    from: String,
    rows: Vec<PerformanceIndicatorsRow>,
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
            file_name: CSV_FILE_NAME.to_string(),
            writer: None,
        }
    }

    /// Start the [`WriterActor`]
    ///
    /// This function is meant to be used directly in the [`WriterActorHandle`].
    async fn start(&mut self) {
        let mut file = File::create(&self.file_name)
            .unwrap_or_else(|_| panic!("Could not open target file \"{}\".", self.file_name));
        let _ = writeln!(&mut file, "{}", CSV_HEADER);
        self.writer = Some(BufWriter::new(file));
        println!("WriterActor is started.");

        self.run().await
    }

    /// Run the [`WriterActor`]
    ///
    /// This function is meant to be used indirectly - only through the [`WriterActor::start`] function
    async fn run(&mut self) -> MsgResponseType {
        println!("WriterActor is running.");

        while let Some(msg) = self.receiver.recv().await {
            self.handle(msg).await;
        }
    }

    /// Stop the [`WriterActor`]
    ///
    /// This function is meant to be called in the [`WriterActor`]'s destructor.
    fn stop(&mut self) {
        // if let Some(writer) = &mut self.writer {
        //     writer
        //         .flush()
        //         .expect("Failed to flush writer. Data loss :(")
        // };

        println!("WriterActor is flushed and properly stopped.");
    }

    /// The [`PerformanceIndicatorsRowsMsg`] message handler for the [`WriterActor`] actor
    async fn handle(&mut self, msg: PerformanceIndicatorsRowsMsg) -> MsgResponseType {
        let from = msg.from;
        let rows = msg.rows;

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

            file.flush().expect("Failed to flush to file. Data loss :/");
        }
    }
}

impl Drop for WriterActor {
    fn drop(&mut self) {
        let _ = self.stop();
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
        let (sender, receiver) = mpsc::channel(MPSC_CHANNEL_CAPACITY);
        let mut actor = WriterActor::new(receiver);
        tokio::spawn(async move { actor.start().await });

        Self { sender }
    }

    /// Send a message to an [`WriterActor`] instance through the [`WriterActorHandle`]
    async fn send(
        &self,
        msg: PerformanceIndicatorsRowsMsg,
    ) -> Result<MsgResponseType, WriterMsgErrorType> {
        Ok(self.sender.send(msg).await?)
    }
}
