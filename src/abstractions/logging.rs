use std::fmt::Debug;
use tokio::sync::broadcast::Receiver;

const LINE_DELIM: &str = "------------------------------------------------------";

pub struct ScopedEventLogger<I, E> {
    process_id: I,
    event_receiver: Receiver<E>,
}

impl<I, E> ScopedEventLogger<I, E>
where
    I: 'static + Send + Debug + Copy,
    E: 'static + Send + Debug + Clone,
{
    pub fn new(process_id: I, event_receiver: Receiver<E>) -> Self {
        Self {
            process_id,
            event_receiver,
        }
    }

    pub fn start(mut self) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Ok(event) = self.event_receiver.recv() => {

                        let log_event = LogEvent {
                            process_id: self.process_id,
                            event: event,
                        };
                        println!("{}\n{:#?}",LINE_DELIM, log_event);
                    }
                }
            }
        });
    }
}

#[derive(Debug)]
struct LogEvent<I, E>
where
    I: Debug,
    E: Debug,
{
    process_id: I,
    event: E,
}
