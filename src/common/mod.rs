use crate::composition_model::{CallbackFn, Module};

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::io::Result;
use tokio::sync::{broadcast::Sender, mpsc};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub component: Component,
    pub event_type: EventType,
    // TODO: add message attribute - pub message: Message
    // TODO: add process attribute - pub process: Option<I>
}

#[derive(Clone, Copy, Debug)]
pub enum Component {
    JobHandler,
    TransformationJobHandler,
    FairLossPointToPointLinks,
}

#[derive(Clone, Copy, Debug)]
pub enum EventType {
    Submit,
    Confirm,
    Send,
    Deliver,
    Broadcast,
}

#[derive(Clone, Copy, Debug)]
pub enum Message {
    Proposal(ProposalType),
}

#[derive(Clone, Copy, Debug)]
pub enum ProposalType {
    Flooding { round: usize, proposal_set: usize },
    Hierarchical { value: usize },
}

//TODO: How do links discover eachother?
pub trait LinkSender<I, T: 'static + Send> {
    fn send(&self, id: I, t: T);
}

pub trait LinkReceiver<T: 'static + Send> {
    // I don't fully understand why I need to add this return signature of Future + Send
    // Could it be possible for someone to implement recv where the returned Future isn't send.
    // What's an example of that?
    // https://stackoverflow.com/questions/78134843/future-cannot-be-sent-between-threads-safely-error
    fn recv(&mut self) -> impl Future<Output = Result<T>> + Send;
}

pub struct LocalProcess;

pub struct LocalProcessReceiver<T> {
    id: Uuid,
    event_receiver: mpsc::Receiver<T>,
}

pub struct LocalProcessSender<T> {
    id: Uuid,
    senders: HashMap<Uuid, mpsc::Sender<T>>,
}

impl LocalProcess {
    pub fn new<T>(
        event_receiver: mpsc::Receiver<T>,
    ) -> (LocalProcessSender<T>, LocalProcessReceiver<T>) {
        let id = Uuid::now_v1(&[1, 2, 3, 4, 5, 6]);

        let sender = LocalProcessSender {
            id,
            senders: HashMap::new(),
        };

        let receiver = LocalProcessReceiver { id, event_receiver };

        (sender, receiver)
    }
}

// TODO: inject random failures to simulate link failures. Should I do Sender and Receiver
impl<T: 'static + Send> LinkSender<Uuid, T> for LocalProcessSender<T> {
    fn send(&self, id: Uuid, t: T) {
        self.senders.get(&id).map(|sender| {
            let sender = sender.clone();
            tokio::spawn(async move { sender.send(t).await })
        });
    }
}

impl<T: 'static + Send> LinkReceiver<T> for LocalProcessReceiver<T> {
    async fn recv(&mut self) -> Result<T> {
        Ok(self.event_receiver.recv().await.unwrap())
    }
}

// Chapter 1 Examples
pub struct JobHandler {
    event_sender: Sender<Event>,
}

impl JobHandler {
    pub fn new(event_sender: Sender<Event>) -> Self {
        Self { event_sender }
    }
}

impl Module<Event> for JobHandler {
    fn handle(&mut self, event: Event) -> Option<CallbackFn> {
        match (event.component, event.event_type) {
            (Component::JobHandler, EventType::Submit) => {
                let event_sender = self.event_sender.clone();
                Some(Box::new(move || {
                    event_sender
                        .send(Event {
                            component: Component::JobHandler,
                            event_type: EventType::Confirm,
                        })
                        .unwrap();
                }))
            }
            _ => None,
        }
    }
}

pub struct TransformationJobHandler {
    event_sender: Sender<Event>,
    handling: bool,
    buffer: VecDeque<usize>,
}

impl TransformationJobHandler {
    pub fn new(event_sender: Sender<Event>) -> Self {
        Self {
            event_sender,
            handling: false,
            buffer: VecDeque::new(),
        }
    }
}

impl Module<Event> for TransformationJobHandler {
    fn handle(&mut self, event: Event) -> Option<CallbackFn> {
        match (event.component, event.event_type) {
            (Component::TransformationJobHandler, EventType::Submit) => {
                self.buffer.push_back(1);
                let event_sender = self.event_sender.clone();
                Some(Box::new(move || {
                    event_sender
                        .send(Event {
                            component: Component::TransformationJobHandler,
                            event_type: EventType::Confirm,
                        })
                        .unwrap();
                }))
            }
            (Component::JobHandler, EventType::Confirm) => {
                self.handling = false;
                None
            }
            _ => None,
        }
    }
}
