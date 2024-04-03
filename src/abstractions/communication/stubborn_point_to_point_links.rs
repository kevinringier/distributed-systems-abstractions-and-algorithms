use std::fmt::Debug;
use tokio::sync::broadcast::Sender;
use tokio_util::sync::CancellationToken;

use crate::abstractions::communication::{LinkReceiver, LinkSender};
use crate::abstractions::composition_model::{CallbackFn, Module};
use crate::abstractions::{Component, Event, EventType, Message};

pub struct StubbornPointToPointLinks<I>
where
    I: Send,
{
    process_id: I,
    event_sender: Sender<Event<I>>,
}

impl<I> StubbornPointToPointLinks<I>
where
    I: 'static + Send + Debug,
{
    pub fn new<LR>(
        cancel_token: CancellationToken,
        process_id: I,
        event_sender: Sender<Event<I>>,
    ) -> Self
    where
        LR: 'static + LinkReceiver<I, (I, Message)> + Send,
    {
        // local event handler
        Self {
            process_id,
            event_sender,
        }
    }

    fn start_timer(cancel_token: CancellationToken, event_sender: Sender<Event<I>>) {
        tokio::spawn({
            async move {
                loop {
                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            return;
                        }
                    }
                }
            }
        });
    }
}

//impl<I> Module<I, Event<I>> for StubbornPointToPointLinks<I>
//where
//    I: 'static + Send + Debug + Copy,
//{
//    fn handle_event(&self, event: Event<I>) -> Option<CallbackFn> {
//        match (
//            event.component,
//            event.event_type,
//            event.process,
//            event.message,
//        ) {
//            (Component::FairLossPointToPointLinks, EventType::Send, Some(process_id), message) => {
//                Some(Box::new(move || {
//                    link_sender.send(process_id, (from_process_id, message));
//                }))
//            }
//            _ => None,
//        }
//    }
//}
