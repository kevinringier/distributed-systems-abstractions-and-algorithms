use std::sync::{Arc, Condvar, Mutex, Once};
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::abstractions::process::{ProcessError, Requestable, Runnable};

pub type CallbackFn = Box<dyn Fn() + Send>;
pub type EventHandlerFn<E> = Box<dyn Fn(E, broadcast::Sender<E>) -> Option<CallbackFn>>;
pub type BoxModule<E> = Box<dyn Module<Event = E> + Send>;

pub struct SoftwareStack<I, E>
where
    E: Clone,
{
    pub process_id: I,
    modules: Vec<BoxModule<E>>,
    event_sender: broadcast::Sender<E>,
    callback_sender: mpsc::Sender<CallbackFn>,
    callback_receiver: mpsc::Receiver<CallbackFn>,
    ready_to_receive_cond: Arc<(Mutex<bool>, Condvar)>,
}

impl<I, E> SoftwareStack<I, E>
where
    E: Clone,
{
    pub fn new(
        process_id: I,
        event_sender: broadcast::Sender<E>,
        callback_sender: mpsc::Sender<CallbackFn>,
        callback_receiver: mpsc::Receiver<CallbackFn>,
    ) -> (Self, SoftwareStackSender<E>) {
        let modules = vec![];
        let ready_to_receive_cond = Arc::new((Mutex::new(false), Condvar::new()));

        let software_stack = Self {
            process_id,
            modules,
            event_sender: event_sender.clone(),
            callback_sender,
            callback_receiver,
            ready_to_receive_cond: ready_to_receive_cond.clone(),
        };

        let software_stack_sender = SoftwareStackSender {
            event_sender,
            ready_to_receive_cond,
            ready_to_receive_once: Once::new(),
        };

        (software_stack, software_stack_sender)
    }
}

impl<I, E> SoftwareStack<I, E>
where
    I: 'static,
    E: 'static + Send + Clone,
{
    // TODO: DElete
    pub async fn run(mut self, cancel_token: CancellationToken) {
        Self::run_modules(
            self.modules,
            cancel_token.clone(),
            &self.event_sender,
            self.callback_sender,
        );

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    return
                }
                Some(callback) = self.callback_receiver.recv() => {
                    callback();
                }
            }
        }
    }

    pub fn add_module(&mut self, module: BoxModule<E>) {
        self.modules.push(module);
    }

    fn run_modules(
        modules: Vec<BoxModule<E>>,
        cancel_token: CancellationToken,
        event_sender: &broadcast::Sender<E>,
        callback_sender: mpsc::Sender<CallbackFn>,
    ) {
        for module in modules {
            module.run(
                cancel_token.clone(),
                event_sender.clone(),
                event_sender.subscribe(),
                callback_sender.clone(),
            );
        }
    }
}

impl<I, E> Runnable for SoftwareStack<I, E>
where
    I: 'static,
    E: 'static + Send + Clone,
{
    fn run(mut self, cancel_token: CancellationToken) -> JoinHandle<Result<(), ProcessError>> {
        Self::run_modules(
            self.modules,
            cancel_token.clone(),
            &self.event_sender,
            self.callback_sender,
        );

        // The software stack is ready to receive once all modules have subscribed to the
        // event_sender.
        let (lock, cvar) = &*self.ready_to_receive_cond;
        let mut started = lock.lock().unwrap();
        *started = true;
        cvar.notify_one();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        return Ok(())
                    }
                    Some(callback) = self.callback_receiver.recv() => {
                        callback();
                    }

                    // TODO:
                    // select on some error condition. could be random, could be an additional
                    // cancel token.
                }
            }
        })
    }
}

pub struct SoftwareStackSender<E> {
    event_sender: broadcast::Sender<E>,
    ready_to_receive_cond: Arc<(Mutex<bool>, Condvar)>,
    ready_to_receive_once: Once,
}

impl<E> Requestable<E> for SoftwareStackSender<E>
where
    E: Clone + Send,
{
    fn emit_request_event(&self, event: E) {
        // Once we've been notified, we can continue to send without checking.
        self.ready_to_receive_once.call_once(|| {
            let (lock, cvar) = &*Arc::clone(&self.ready_to_receive_cond);
            let mut started = lock.lock().unwrap();
            while !*started {
                started = cvar.wait(started).unwrap();
            }
        });

        let _ = self.event_sender.send(event);
    }
}

impl<E> Clone for SoftwareStackSender<E> {
    fn clone(&self) -> Self {
        Self {
            event_sender: self.event_sender.clone(),
            ready_to_receive_cond: self.ready_to_receive_cond.clone(),
            ready_to_receive_once: Once::new(),
        }
    }
}

pub trait Module {
    type Event;
    fn run(
        // https://doc.rust-lang.org/reference/items/traits.html#object-safety
        // Must be Box<Self> to be a dispatchable function.
        self: Box<Self>,
        cancel_token: CancellationToken,
        event_sender: broadcast::Sender<Self::Event>,
        event_receiver: broadcast::Receiver<Self::Event>,
        callback_sender: mpsc::Sender<CallbackFn>,
    );
}

// TODO: How to handle local events
// thought
// this may not belong as an abstraction
// local events need their own thread running to check the conditions of
// I will let the bespoke modules determine when they are ready from recovery to start accepting
// events. Just keep a local state that determines if recovered and only accept recovery messages
// if false.
