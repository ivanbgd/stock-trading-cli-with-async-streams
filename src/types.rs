use tokio::sync::mpsc::error::SendError;

use crate::my_async_actors::{ActorMessage, PerformanceIndicatorsRowsMsg};

pub type MsgResponseType = ();
pub type UniversalMsgErrorType = SendError<ActorMessage>;
pub type WriterMsgErrorType = SendError<PerformanceIndicatorsRowsMsg>;
