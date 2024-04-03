use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::abstractions::communication::LocalProcessFactory;

pub struct CrashStop<E> {
    event_sender: broadcast::Sender<E>,
}

impl<E> CrashStop<E> {
    pub fn run<R: Runnable>(
        cancel_token: CancellationToken,
        runnable: R,
    ) -> JoinHandle<Result<(), ProcessError>> {
        runnable.run(cancel_token)
    }

    pub fn emit_event(&self, event: E) {
        let _ = self.event_sender.send(event);
    }
}

pub struct CrashRecovery<I, M, E> {
    id: I,
    local_process_factory: Arc<Mutex<LocalProcessFactory<I, M>>>,
    on_recover_events: Vec<E>,
    internal_event_receiver: mpsc::Receiver<E>,
}
impl<I, M, E> CrashRecovery<I, M, E>
where
    I: Copy,
    E: 'static + Send + Copy,
{
    pub fn new(
        id: I,
        local_process_factory: Arc<Mutex<LocalProcessFactory<I, M>>>,
    ) -> (Self, ProcessEventSender<E>) {
        let (internal_event_sender, internal_event_receiver) = mpsc::channel(1);

        let process = Self {
            id,
            local_process_factory,
            on_recover_events: Vec::new(),
            internal_event_receiver,
        };

        let process_event_sender = ProcessEventSender {
            event_sender: internal_event_sender,
        };

        (process, process_event_sender)
    }

    pub async fn run<ES: 'static + Requestable<E>, R: Runnable>(
        &mut self,
        software_stack_creator_fn: RecoverableRunnerCreatorFn<I, M, E, ES, R>,
        cancel_token: CancellationToken,
    ) {
        let mut restarted = true;
        loop {
            let recoverable_runnable: RecoverableRunnable<E, ES, R>;
            {
                let mut guard = self.local_process_factory.lock().unwrap();
                recoverable_runnable = software_stack_creator_fn(self.id, &mut guard);
            }

            let runnable_handle = recoverable_runnable.runnable.run(cancel_token.clone());
            tokio::pin!(runnable_handle);

            if restarted {
                self.emit_recover_events(recoverable_runnable.event_sender.clone());
            }

            loop {
                tokio::select! {
                    Ok(run_result) = &mut runnable_handle => {
                        match run_result {
                            Ok(()) => return,
                            Err(_) => {
                                restarted = true;
                                break;
                            }
                        }
                    }
                    Some(event) = self.internal_event_receiver.recv() => {
                        recoverable_runnable.event_sender.emit_request_event(event);
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    fn register_on_recover(&mut self, mut events: Vec<E>) {
        self.on_recover_events.append(&mut events);
    }

    fn emit_recover_events<ES: 'static + Requestable<E>>(&mut self, event_sender: ES) {
        for event in self.on_recover_events.iter() {
            let event = event.clone();
            let event_sender = event_sender.clone();
            tokio::spawn(async move {
                let _ = event_sender.emit_request_event(event.clone());
            });
        }
    }
}

pub struct ProcessEventSender<E> {
    event_sender: mpsc::Sender<E>,
}

impl<E> ProcessEventSender<E> {
    pub async fn emit_request_event(&self, event: E) {
        let _ = self.event_sender.send(event).await;
    }
}

pub struct RecoverableRunnable<E, ES: Requestable<E>, R: Runnable> {
    event_sender: ES,
    runnable: R,
    _phatom_event: std::marker::PhantomData<E>,
}

impl<E, ES: Requestable<E>, R: Runnable> RecoverableRunnable<E, ES, R> {
    pub fn new(event_sender: ES, runnable: R) -> Self {
        Self {
            event_sender,
            runnable,
            _phatom_event: std::marker::PhantomData,
        }
    }
}

pub type RecoverableRunnerCreatorFn<I, M, E, ES, R> =
    Box<dyn Fn(I, &mut LocalProcessFactory<I, M>) -> RecoverableRunnable<E, ES, R> + Send>;

pub trait Runnable {
    fn run(self, cancel_token: CancellationToken) -> JoinHandle<Result<(), ProcessError>>;
}

pub trait Requestable<E>: Clone + Send {
    fn emit_request_event(&self, event: E);
}

pub enum ProcessError {
    Crash,
}

pub struct ID {
    id: usize,
}

impl ID {
    pub fn new() -> Self {
        Self { id: 0 }
    }
    pub fn get(&mut self) -> usize {
        let cur_id = self.id;
        self.id += 1;

        cur_id
    }
}
