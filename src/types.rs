use tokio::sync::mpsc::error::SendError;

use crate::my_async_actors::{ActorMessage, CollectionActorMsg, PerformanceIndicatorsRowsMsg};

pub type MsgResponseType = ();
pub type UniversalMsgErrorType = SendError<ActorMessage>;
pub type WriterMsgErrorType = SendError<PerformanceIndicatorsRowsMsg>;
pub type CollectionMsgErrorType = SendError<CollectionActorMsg>;
