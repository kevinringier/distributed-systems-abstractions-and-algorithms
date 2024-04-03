use std::sync::Arc;
use std::thread;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::abstractions::communication::{LinkReceiver, LinkSender};
use crate::abstractions::composition_model::{CallbackFn, Module};
use crate::abstractions::{Component, Event, EventType, Message};

pub struct FairLossPointToPointLinks<LS, LR, I>
where
    I: 'static + Send,
    LS: LinkSender<I, (I, Message)>,
    LR: LinkReceiver<I, (I, Message)>,
{
    process_id: I,
    link_sender: Arc<LS>,
    link_receiver: LR,
    internal_event_sender: Arc<mpsc::UnboundedSender<InternalEvent<I>>>,
    internal_event_receiver: mpsc::UnboundedReceiver<InternalEvent<I>>,
}

impl<LS, LR, I> FairLossPointToPointLinks<LS, LR, I>
where
    I: 'static + Send + Copy,
    LS: 'static + LinkSender<I, (I, Message)> + Send + Sync,
    LR: 'static + LinkReceiver<I, (I, Message)> + Send,
{
    pub fn new(process_id: I, link_sender: LS, link_receiver: LR) -> Self
where {
        let (sender, receiver) = mpsc::unbounded_channel();

        Self {
            process_id,
            link_sender: Arc::new(link_sender),
            link_receiver,
            internal_event_sender: Arc::new(sender),
            internal_event_receiver: receiver,
        }
    }

    fn start_link_receiver(
        cancel_token: CancellationToken,
        internal_event_sender: Arc<mpsc::UnboundedSender<InternalEvent<I>>>,
        mut link_receiver: LR,
    ) where
        LR: 'static + LinkReceiver<I, (I, Message)> + Send,
    {
        tokio::spawn({
            async move {
                loop {
                    tokio::select! {
                        Ok((from_id, message)) = link_receiver.recv() => {
                            let internal_event = InternalEvent::MessageReceived {
                                from_id, message,
                            };
                            internal_event_sender.send(internal_event).unwrap();
                        }
                        _ = cancel_token.cancelled() => {
                            return;
                        }
                    }
                }
            }
        });
    }

    fn handle_external_event(
        self_process_id: I,
        link_sender: Arc<LS>,
        event: Event<I>,
    ) -> Option<CallbackFn> {
        match (
            event.component,
            event.event_type,
            event.process,
            event.message,
        ) {
            (Component::FairLossPointToPointLinks, EventType::Send, Some(process_id), message) => {
                Some(Box::new(move || {
                    link_sender.send(process_id, (self_process_id, message));
                }))
            }
            _ => None,
        }
    }

    fn handle_internal_event(
        event_sender: broadcast::Sender<Event<I>>,
        event: InternalEvent<I>,
    ) -> Option<CallbackFn> {
        match event {
            InternalEvent::MessageReceived { from_id, message } => {
                let event = Event {
                    component: Component::FairLossPointToPointLinks,
                    event_type: EventType::Deliver,
                    process: Some(from_id),
                    message: message,
                };

                Some(Box::new(move || {
                    let _ = event_sender.send(event);
                }))
            }
        }
    }
}

impl<LS, LR, I> Module for FairLossPointToPointLinks<LS, LR, I>
where
    I: 'static + Send + Copy,
    LS: 'static + LinkSender<I, (I, Message)> + Send + Sync,
    LR: 'static + LinkReceiver<I, (I, Message)> + Send,
{
    type Event = Event<I>;
    fn run(
        // https://doc.rust-lang.org/reference/items/traits.html#object-safety
        // Must be Box<Self> to be a dispatchable function.
        self: Box<Self>,
        cancel_token: CancellationToken,
        event_sender: tokio::sync::broadcast::Sender<Self::Event>,
        mut event_receiver: tokio::sync::broadcast::Receiver<Self::Event>,
        callback_sender: mpsc::Sender<CallbackFn>,
    ) {
        // start local internal event emitters
        tokio::spawn({
            let cancel_token = cancel_token.clone();
            let internal_event_sender = self.internal_event_sender.clone();
            let link_receiver = self.link_receiver;
            async move {
                Self::start_link_receiver(cancel_token, internal_event_sender, link_receiver);
            }
        });

        tokio::spawn({
            let self_process_id = self.process_id;
            let link_sender = self.link_sender;
            let mut internal_event_receiver = self.internal_event_receiver;
            async move {
                loop {
                    let callback_opt = tokio::select! {
                        Ok(external_event) = event_receiver.recv() => {
                            Self::handle_external_event(self_process_id, link_sender.clone(), external_event)
                        }
                        Some(internal_event) = internal_event_receiver.recv() => {
                            Self::handle_internal_event(event_sender.clone(), internal_event)
                        }
                    };

                    match callback_opt {
                        Some(callback) => {
                            let _ = callback_sender.send(callback).await;
                        }
                        None => (),
                    }
                }
            }
        });
    }
}

enum InternalEvent<I>
where
    I: Send,
{
    MessageReceived { from_id: I, message: Message },
}
