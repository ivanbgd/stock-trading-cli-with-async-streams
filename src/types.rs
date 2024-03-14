use tokio::sync::mpsc::error::SendError;

use crate::my_actors::ActorMessage;

pub type MsgResponseType = ();
pub type MsgErrorType = SendError<ActorMessage>;
