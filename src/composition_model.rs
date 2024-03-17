use tokio::sync::{broadcast, mpsc, Notify};
use tokio_util::sync::CancellationToken;

pub type CallbackFn = Box<dyn Fn() + Send>;
pub type EventHandlerFn<E> = Box<dyn Fn(E, broadcast::Sender<E>) -> Option<CallbackFn>>;
pub type BoxModule<E> = Box<dyn Module<E> + Send>;

pub struct SoftwareStack<E, F, M>
where
    E: Clone,
{
    modules: Vec<M>,
    event_sender: broadcast::Sender<E>,
    callback_sender: mpsc::Sender<F>,
    callback_receiver: mpsc::Receiver<F>,
    initial_event_notifier: Notify,
}

impl<E, F, M> SoftwareStack<E, F, M>
where
    E: Clone,
{
    pub fn new(
        event_sender: broadcast::Sender<E>,
        callback_sender: mpsc::Sender<F>,
        callback_receiver: mpsc::Receiver<F>,
    ) -> Self {
        let modules = vec![];
        let initial_event_notifier = Notify::new();

        Self {
            modules,
            event_sender,
            callback_sender,
            callback_receiver,
            initial_event_notifier,
        }
    }
}

impl<E> SoftwareStack<E, CallbackFn, BoxModule<E>>
where
    E: 'static + Send + Clone,
{
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

    fn run_module(
        mut module: BoxModule<E>,
        cancel_token: CancellationToken,
        mut event_receiver: broadcast::Receiver<E>,
        callback_sender: mpsc::Sender<CallbackFn>,
    ) {
        tokio::spawn({
            async move {
                loop {
                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            return
                        }
                        Ok(event) = event_receiver.recv() => {
                            match module.handle(event) {
                                Some(callback) => {
                                    let _ = callback_sender.send(callback).await;
                                },
                                _ => (),
                            }

                        }
                    }
                }
            }
        });
    }

    fn run_modules(
        modules: Vec<BoxModule<E>>,
        cancel_token: CancellationToken,
        event_sender: &broadcast::Sender<E>,
        callback_sender: mpsc::Sender<CallbackFn>,
    ) {
        for module in modules {
            Self::run_module(
                module,
                cancel_token.clone(),
                event_sender.subscribe(),
                callback_sender.clone(),
            );
        }
    }
}

pub trait Module<E> {
    fn handle(&mut self, event: E) -> Option<CallbackFn>;
}
