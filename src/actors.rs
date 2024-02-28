use actix::{Actor, Context, Handler, Message};

pub struct MultiActor {}

impl Actor for MultiActor {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Msg<'a>(pub Vec<&'a str>);

impl Handler<Msg<'_>> for MultiActor {
    type Result = ();

    fn handle(&mut self, msg: Msg, _ctx: &mut Self::Context) -> Self::Result {
        todo!()
    }
}
