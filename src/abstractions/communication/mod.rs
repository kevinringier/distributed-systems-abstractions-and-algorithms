use core::hash::Hash;
use std::sync::Arc;
use std::sync::Mutex;
use std::{collections::HashMap, future::Future, io::Result};
use tokio::sync::mpsc;

pub mod fair_loss_point_to_point_links;
pub mod stubborn_point_to_point_links;

pub trait LinkSender<I, T: 'static + Send> {
    fn send(&self, id: I, t: T);
}

pub trait LinkReceiver<I, T>
where
    I: 'static + Send,
    T: 'static + Send,
{
    // https://stackoverflow.com/questions/78134843/future-cannot-be-sent-between-threads-safely-error
    fn recv(&mut self) -> impl Future<Output = Result<T>> + Send;
}

pub struct LocalProcessFactory<I, T> {
    active_link_senders: HashMap<I, mpsc::Sender<(I, T)>>,
    active_local_process_senders: HashMap<I, LocalProcessSender<I, T>>,
}

impl<I, T> LocalProcessFactory<I, T>
where
    I: PartialEq + Eq + Hash + Copy,
{
    pub fn new() -> Self {
        Self {
            active_link_senders: HashMap::new(),
            active_local_process_senders: HashMap::new(),
        }
    }

    pub fn new_local_process_link(
        &mut self,
        id: I,
    ) -> (LocalProcessSender<I, T>, LocalProcessReceiver<I, T>) {
        let (inner_sender, inner_receiver) = mpsc::channel(100);
        let sender = LocalProcessSender {
            id,
            senders: Arc::new(Mutex::new(HashMap::new())),
        };
        let receiver = LocalProcessReceiver {
            id,
            event_receiver: inner_receiver,
        };

        for (_, local_process_sender) in self.active_local_process_senders.iter_mut() {
            local_process_sender
                .senders
                .lock()
                .unwrap()
                .insert(id, inner_sender.clone());
        }

        for (id, link_sender) in self.active_link_senders.iter() {
            sender
                .senders
                .lock()
                .unwrap()
                .insert(id.clone(), link_sender.clone());
        }

        self.active_link_senders.insert(id, inner_sender);
        self.active_local_process_senders.insert(id, sender.clone());

        (sender, receiver)
    }
}

pub struct LocalProcessSender<I, T> {
    id: I,
    senders: Arc<Mutex<HashMap<I, mpsc::Sender<(I, T)>>>>,
}

// TODO: inject random failures to simulate link failures. Should I do Sender and Receiver
impl<I, T> LinkSender<I, (I, T)> for LocalProcessSender<I, T>
where
    I: 'static + Send + PartialEq + Eq + core::hash::Hash,
    T: 'static + Send,
{
    fn send(&self, id: I, t: (I, T)) {
        self.senders.lock().unwrap().get(&id).map(|sender| {
            let sender = sender.clone();
            // note: calling an async fn within map may only work when spawing a thread.
            tokio::spawn(async move { sender.send(t).await })
        });
    }
}

impl<I, T> Clone for LocalProcessSender<I, T>
where
    I: Copy,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            senders: self.senders.clone(),
        }
    }
}

pub struct LocalProcessReceiver<I, T> {
    id: I,
    event_receiver: mpsc::Receiver<(I, T)>,
}

impl<I, T> LinkReceiver<I, (I, T)> for LocalProcessReceiver<I, T>
where
    I: 'static + Send,
    T: 'static + Send,
{
    async fn recv(&mut self) -> Result<(I, T)> {
        Ok(self.event_receiver.recv().await.unwrap())
    }
}
