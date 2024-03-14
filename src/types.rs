use tokio::sync::mpsc::error::SendError;

use crate::my_async_actors::ActorMessage;

pub type MsgResponseType = ();
pub type MsgErrorType = SendError<ActorMessage>;
