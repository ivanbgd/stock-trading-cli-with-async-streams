use tokio::sync::{mpsc, oneshot};

enum ActorMessage {
    GetUniqueID { respond_to: oneshot::Sender<u32> },
}

struct MyActor {
    receiver: mpsc::Receiver<ActorMessage>,
    next_id: u32,
}

impl MyActor {
    fn new(receiver: mpsc::Receiver<ActorMessage>) -> Self {
        Self {
            receiver,
            next_id: 0,
        }
    }

    fn handle(&mut self, msg: ActorMessage) {
        // todo move to run - maybe
        match msg {
            ActorMessage::GetUniqueID { respond_to } => {
                self.next_id += 1;

                // The `let _ =` ignores any errors when sending.
                // An error can happen if the `select!` macro is used
                // to cancel waiting for the response.
                let _ = respond_to.send(self.next_id); // todo perhaps handle error
            }
        }
    }
}

async fn run_my_actor(mut actor: MyActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle(msg);
    }
}

#[derive(Clone)]
pub struct MyActorHandle {
    sender: mpsc::Sender<ActorMessage>,
}

impl MyActorHandle {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(8);
        let actor = MyActor::new(receiver);
        tokio::spawn(run_my_actor(actor));

        Self { sender }
    }

    pub async fn get_unique_id(&self) -> u32 {
        let (sender, receiver) = oneshot::channel();
        let msg = ActorMessage::GetUniqueID { respond_to: sender };

        // Ignore send errors. If this send fails, so does the
        // recv.await below. There's no reason to check for the
        // same failure twice.
        let _ = self.sender.send(msg).await;
        receiver.await.expect("Actor task has been killed")
    }
}
