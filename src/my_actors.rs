use tokio::sync::{mpsc, oneshot};

enum ActorMessage {
    GetUniqueID { respond_to: oneshot::Sender<u32> },
}

/// A single (universal) type of actor
///
/// It can receive and handle any of the three possible message types.
///
/// It is not made public on purpose.
///
/// It can only be created through [`ActorHandle`], which is public.
struct Actor {
    receiver: mpsc::Receiver<ActorMessage>,
    next_id: u32,
}

impl Actor {
    /// Create a new actor
    fn new(receiver: mpsc::Receiver<ActorMessage>) -> Self {
        Self {
            receiver,
            next_id: 0,
        }
    }

    /// Run the actor
    async fn run(&mut self) {
        while let Some(msg) = self.receiver.recv().await {
            self.handle(msg);
        }
    }

    fn handle(&mut self, msg: ActorMessage) {
        match msg {
            ActorMessage::GetUniqueID { respond_to } => {
                self.next_id += 1;

                // The `let _ =` ignores any errors when sending.
                // An error can happen if the `select!` macro is used
                // to cancel waiting for the response.
                let _ = respond_to.send(self.next_id);
            }
        }
    }
}

/// A handle for the [`Actor`]
///
/// Only the handle is public; the [`Actor`] isn't.
///
/// We can only create [`Actor`]s through the [`ActorHandle`].
///
/// It contains the `sender` field, which represents
/// a sender of the [`ActorMessage`] in an MPSC channel.
///
/// The handle is the sender, and the actor is the receiver
/// of a message in the channel.
///
/// We only create a single [`Actor`] instance in an [`ActorHandle`].
#[derive(Clone)]
pub struct ActorHandle {
    sender: mpsc::Sender<ActorMessage>,
}

impl ActorHandle {
    /// Create a new [`ActorHandle`]
    ///
    /// This function creates a single [`Actor`] instance,
    /// and a MPSC channel for communicating to the actor.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(MPSC_CHANNEL_CAPACITY);
        let mut actor = Actor::new(receiver);
        tokio::spawn(async move { actor.run().await });

        Self { sender }
    }

    pub async fn get_unique_id(&self) -> u32 {
        let (sender, receiver) = oneshot::channel();
        let msg = ActorMessage::GetUniqueID { respond_to: sender };

        // Ignore send errors. If this sending fails, so does the
        // recv.await below. There's no reason to check for the
        // same failure twice.
        let _ = self.sender.send(msg).await;
        receiver.await.expect("Actor task has been killed")
    }

    pub async fn get_unique_id(&self) -> u32 {
        // Create an oneshot channel for sending the response back.
        // This is not the same channel as the MPSC one.
        // We use the MPSC channel to send a message to an actor.
        // We pack the `send` in it, and that's this actor handler.
        // That's a return address for the actor, which is a receiver.
        // This actor handler is also a `recv` of the response message from the actor.
        let (send, recv) = oneshot::channel();

        let msg = ActorMessage::GetUniqueID { respond_to: send };

        // Ignore send errors. If this sending fails, so does the
        // recv.await below. There's no reason to check for the
        // same failure twice.
        let _ = self.sender.send(msg).await;
        recv.await.expect("Actor task has been killed")
    }
}
