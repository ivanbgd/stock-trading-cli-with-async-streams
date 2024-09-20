use tokio::sync::mpsc::error::SendError;

use crate::my_async_actors::{
    ActorMessage, CollectionActorMsg, PerformanceIndicatorsRow, PerformanceIndicatorsRowsMsg,
};

pub type MsgResponseType = ();
pub type UniversalMsgErrorType = SendError<ActorMessage>;
pub type WriterMsgErrorType = SendError<PerformanceIndicatorsRowsMsg>;
pub type CollectionMsgErrorType = SendError<CollectionActorMsg>;

/// A single iteration of the main loop, which contains processed data
/// for all S&P 500 symbols
pub type Batch = Vec<PerformanceIndicatorsRow>;

/// A response for the web server which contains the requested last `n`
/// batches of processed symbol data
pub type TailResponse = Vec<Batch>;
