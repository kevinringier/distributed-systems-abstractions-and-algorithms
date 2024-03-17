use distributed_systems_abstractions_and_algorithms::{
    common::{Component, Event, EventType, JobHandler, TransformationJobHandler},
    composition_model::{BoxModule, CallbackFn, SoftwareStack},
};

use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    let (event_sender, _) = broadcast::channel::<Event>(100);
    let (callback_sender, callback_receiver) = mpsc::channel(100);

    let mut software_stack = SoftwareStack::<Event, CallbackFn, BoxModule<Event>>::new(
        event_sender.clone(),
        callback_sender.clone(),
        callback_receiver,
    );

    let job_handler: BoxModule<Event> = Box::new(JobHandler::new(event_sender.clone()));
    software_stack.add_module(job_handler);

    let transformation_job_handler: BoxModule<Event> =
        Box::new(TransformationJobHandler::new(event_sender.clone()));
    software_stack.add_module(transformation_job_handler);

    let cancel_token = CancellationToken::new();

    tokio::spawn({
        let cancel_token = cancel_token.clone();
        async move {
            software_stack.run(cancel_token).await;
        }
    });

    let init_event = Event {
        component: Component::JobHandler,
        event_type: EventType::Submit,
    };

    std::thread::sleep(std::time::Duration::from_secs(5));

    event_sender.send(init_event).unwrap();

    tokio::select! {
        signal_result = tokio::signal::ctrl_c() => {
            match signal_result {
                Ok(()) => cancel_token.cancel(),
                Err(err) => {
                    eprintln!("Unable to listen for shutdown signal: {}", err)
                }
            }
        }

    }
}

// TODO:
// - logging and/or debug mode to watch events and actions in a readable format.
//  - can this be generalized, abstracted behind a type, i.e. logging/debugging software_statck, or
//  module? and the debug/log statements don't clutter the actual code.
//  - Log/debug events that are generated to the event queue
//  - log/debug the callbacks being triggered. This may require a refactor to name the sections
//  being executed
// - handle the race condition when no event is in the queue
// - handle local condition events
// - refactor looping in run func. too many lines that can be refactored for better readability
