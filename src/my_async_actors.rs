use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use yahoo_finance_api as yahoo;

use crate::async_signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};
use crate::constants::{CSV_FILE_NAME, CSV_HEADER, WINDOW_SIZE};
use crate::types::MsgResponseType;

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
    fn new() -> Self;

    /// Start the [`Actor`]
    async fn start(&mut self) {}

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
pub(crate) trait ActorHandle<R> {
    /// The type [`Self::Msg`] represents an incoming message type.
    type Msg;

    /// Create a new [`ActorHandle`]
    ///
    /// This function creates a single [`Actor`] instance.
    ///
    /// # Panics
    ///
    /// Panics if it can't run the actor.
    fn new() -> Self;

    /// Send a message to an [`Actor`] instance through the [`ActorHandle`]
    async fn send(&mut self, msg: Self::Msg) -> R;
}

// ============================================================================
//
//         [`QuoteRequestsMsg`], [`FetchActor`], [`FetchActorHandle`]
//
// ============================================================================

/// The [`QuoteRequestsMsg`] message
///
/// It contains a chunk of symbols, and `from` and `to` fields.
///
/// It also contains a [`WriterActor`] handle.
///
/// There is no expected response.
pub struct QuoteRequestsMsg {
    pub symbols: Vec<String>,
    pub from: OffsetDateTime,
    pub to: OffsetDateTime,
    pub writer_handle: WriterActorHandle,
}

/// A universal (general) type of actor
///
/// It can receive and handle two message types.
///
/// It was not made public on purpose.
///
/// It can only be created through [`FetchActorHandle`], which is public.
#[derive(Clone)]
struct FetchActor {}

/// Actor that downloads stock data for a specified symbol and period
///
/// It is stateless - it doesn't contain any user data.
impl Actor<MsgResponseType> for FetchActor {
    type Msg = QuoteRequestsMsg;

    /// Create a new [`FetchActor`]
    fn new() -> Self {
        Self {}
    }

    /// The [`QuoteRequestsMsg`] message handler for the fetch [`FetchActor`] actor
    ///
    /// Creates a new [`ProcesssorActor`] and sends it a [`SymbolsClosesMsg`] message.
    ///
    /// The message contains a hash map of `symbols` and associated `Vec<f64>` with closing prices for that symbol
    /// in case there was no error when fetching the data, or an empty vector in case of an error,
    /// in which case it prints the error message to `stderr`.
    ///
    /// So, in case of an API error for a symbol, when trying to fetch its data,
    /// we don't break the program but rather continue.
    async fn handle(&mut self, msg: QuoteRequestsMsg) -> MsgResponseType {
        let symbols = msg.symbols;
        let from = msg.from;
        let to = msg.to;
        let writer_handle = msg.writer_handle;

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

        let symbols_closes_msg = SymbolsClosesMsg {
            symbols_closes,
            from,
            writer_handle,
        };

        // Create another Actor and send it the message.
        let mut proc_handle = ProcessorActorHandle::new();
        proc_handle.send(symbols_closes_msg).await
    }
}

impl FetchActor {
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

/// A handle for the [`FetchActor`]
///
/// Only the handle is public; the [`FetchActor`] isn't.
///
/// We can only create [`FetchActor`]s through the [`FetchActorHandle`].
///
/// It contains the `actor` field.
///
/// We only create a single [`FetchActor`] instance in an [`FetchActorHandle`].
#[derive(Clone)]
pub struct FetchActorHandle {
    actor: FetchActor,
}

impl ActorHandle<MsgResponseType> for FetchActorHandle {
    type Msg = QuoteRequestsMsg;

    /// Create a new [`FetchActorHandle`]
    ///
    /// This function creates a single [`FetchActor`] instance,
    /// and maintains a reference to the actor, so we can send it
    /// messages.
    ///
    /// It also starts (runs) the actor.
    ///
    /// # Panics
    ///
    /// Panics if it can't run the actor.
    fn new() -> Self {
        let mut actor = FetchActor::new();
        tokio::spawn(async move { actor.start().await });

        Self { actor }
    }

    /// Send a message to a [`FetchActor`] instance through the [`FetchActorHandle`]
    async fn send(&mut self, msg: QuoteRequestsMsg) -> MsgResponseType {
        self.actor.handle(msg).await;
    }
}

// ============================================================================
//
//      [`SymbolsClosesMsg`], [`ProcessorActor`], [`ProcessorActorHandle`]
//
// ============================================================================

/// The [`SymbolsClosesMsg`] message
///
/// It contains a hash map of `symbols` and associated `Vec<f64>` with closing prices for that symbol,
/// and the starting date and time `from` field.
///
/// It also contains a [`WriterActor`] address.
///
/// There is no expected response.
struct SymbolsClosesMsg {
    pub symbols_closes: HashMap<String, Vec<f64>>,
    pub from: OffsetDateTime,
    pub writer_handle: WriterActorHandle,
}

/// A universal (general) type of actor
///
/// It can receive and handle two message types.
///
/// It was not made public on purpose.
///
/// It can only be created through [`ProcessorActorHandle`], which is public.
#[derive(Clone)]
struct ProcessorActor {}

/// Actor that downloads stock data for a specified symbol and period
///
/// It is stateless - it doesn't contain any user data.
impl Actor<MsgResponseType> for ProcessorActor {
    type Msg = SymbolsClosesMsg;

    /// Create a new [`ProcessorActor`]
    fn new() -> Self {
        Self {}
    }

    /// The [`SymbolsClosesMsg`] message handler for the [`ProcessorActor`] actor
    ///
    /// Sends a [`PerformanceIndicatorsRowsMsg`] message to the [`WriterActor`],
    /// whose address it gets from the [`SymbolsClosesMsg`] message.
    async fn handle(&mut self, msg: SymbolsClosesMsg) -> MsgResponseType {
        let symbols_closes = msg.symbols_closes;
        let from = msg.from;
        let mut writer_handle = msg.writer_handle;

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

                let row = crate::my_async_actors::PerformanceIndicatorsRow {
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
        writer_handle.send(perf_ind_msg).await
    }
}

/// A handle for the [`ProcessorActor`]
///
/// Only the handle is public; the [`ProcessorActor`] isn't.
///
/// We can only create [`ProcessorActor`]s through the [`ProcessorActorHandle`].
///
/// It contains the `actor` field.
///
/// We only create a single [`ProcessorActor`] instance in an [`ProcessorActorHandle`].
#[derive(Clone)]
struct ProcessorActorHandle {
    actor: ProcessorActor,
}

impl ActorHandle<MsgResponseType> for ProcessorActorHandle {
    type Msg = SymbolsClosesMsg;

    /// Create a new [`ProcessorActorHandle`]
    ///
    /// This function creates a single [`ProcessorActor`] instance,
    /// and maintains a reference to the actor, so we can send it
    /// messages.
    ///
    /// It also starts (runs) the actor.
    ///
    /// # Panics
    ///
    /// Panics if it can't run the actor.
    fn new() -> Self {
        let mut actor = ProcessorActor::new();
        tokio::spawn(async move { actor.start().await });

        Self { actor }
    }

    /// Send a message to a [`ProcessorActor`] instance through the [`ProcessorActorHandle`]
    async fn send(&mut self, msg: SymbolsClosesMsg) -> MsgResponseType {
        let mut actor = ProcessorActor::new();
        tokio::spawn(async move { actor.handle(msg).await });
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
#[derive(Clone)] // "the trait `Clone` is not implemented for `std::io::BufWriter<std::fs::File>`"
                 // We only have a single [`WriterActor`] in our app, but its handle needs to be cloned in main logic
                 // (in the loop), hence it needs to be `Clone`.
struct WriterActor {
    pub file_name: String,
    pub writer: Option<BufWriter<File>>,
}

impl Actor<MsgResponseType> for WriterActor {
    type Msg = PerformanceIndicatorsRowsMsg;

    /// Create a new [`WriterActor`]
    fn new() -> Self {
        Self {
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
/// It contains the `actor` field.
///
/// We only create a single [`WriterActor`] instance in a [`WriterActorHandle`].
#[derive(Clone)]
pub struct WriterActorHandle {
    actor: WriterActor,
}

impl ActorHandle<MsgResponseType> for WriterActorHandle {
    type Msg = PerformanceIndicatorsRowsMsg;

    /// Create a new [`WriterActorHandle`]
    ///
    /// This function creates a single [`WriterActor`] instance,
    /// and maintains a reference to the actor, so we can send it
    /// messages.
    ///
    /// It also starts (runs) the actor.
    ///
    /// # Panics
    ///
    /// Panics if it can't run the actor.
    fn new() -> Self {
        let mut actor = WriterActor::new();
        tokio::spawn(async move { actor.start().await });

        Self { actor }
    }

    /// Send a message to an [`WriterActor`] instance through the [`WriterActorHandle`]
    async fn send(&mut self, msg: PerformanceIndicatorsRowsMsg) -> MsgResponseType {
        self.actor.handle(msg).await;
    }
}
