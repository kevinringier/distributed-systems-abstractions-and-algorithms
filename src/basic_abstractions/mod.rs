use crate::common::{Component, Event, EventType, LinkReceiver, LinkSender, LocalProcess, Message};
use crate::composition_model::{CallbackFn, Module};

use std::marker::PhantomData;
use tokio::sync::broadcast::Sender;

struct FairLossPointToPointLinks<LS, I, M>
where
    LS: LinkSender<I, M>,
    M: 'static + Send,
{
    event_sender: Sender<Event>,
    link_sender: LS,
    phantom_i: PhantomData<I>,
    phantom_m: PhantomData<M>,
}

impl<LS, I, M> FairLossPointToPointLinks<LS, I, M>
where
    LS: LinkSender<I, M>,
    M: 'static + Send,
{
    pub fn new<LR>(event_sender: Sender<Event>, link_sender: LS, mut link_receiver: LR) -> Self
    where
        LR: 'static + LinkReceiver<M> + Send,
    {
        // TODO: refactor out and add cancel case to select
        tokio::spawn({
            let event_sender = event_sender.clone();
            async move {
                loop {
                    tokio::select! {
                        //TODO: add message when added to event
                        Ok(message) = link_receiver.recv() => {
                            let event = Event {
                                component: Component::FairLossPointToPointLinks,
                                event_type: EventType::Deliver,
                            };
                            event_sender.send(event);

                        }
                    }
                }
            }
        });

        Self {
            event_sender,
            link_sender,
            phantom_i: PhantomData,
            phantom_m: PhantomData,
        }
    }

    // TODO:
    fn send() {}
}

impl<P, I> Module<Event> for FairLossPointToPointLinks<P, I, Message>
where
    P: LinkSender<I, Message>,
{
    fn handle(&mut self, event: Event) -> Option<CallbackFn> {
        match (event.component, event.event_type) {
            (Component::FairLossPointToPointLinks, EventType::Send) => {
                // send requested process/computer
                None
            }
            _ => None,
        }
    }
}
