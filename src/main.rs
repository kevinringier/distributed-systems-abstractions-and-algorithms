use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use distributed_systems_abstractions_and_algorithms::abstractions::{
    communication::fair_loss_point_to_point_links::FairLossPointToPointLinks,
    communication::LocalProcessFactory,
    composition_model::{BoxModule, CallbackFn, SoftwareStack},
    process::{CrashRecovery, RecoverableRunnable, ID},
    {composition_model::SoftwareStackSender, logging::ScopedEventLogger},
    {Component, Event, EventType, JobType, Message},
};

#[tokio::main]
async fn main() {
    let runnable_creator = Box::new(
        |id: usize,
         local_process_factory: &mut LocalProcessFactory<usize, Message>|
         -> RecoverableRunnable<
            Event<usize>,
            SoftwareStackSender<Event<usize>>,
            SoftwareStack<usize, Event<usize>>,
        > {
            let ct = CancellationToken::new();
            let (event_sender, event_receiver) = broadcast::channel::<Event<usize>>(100);
            let logger_1 = ScopedEventLogger::new(id, event_receiver);

            logger_1.start();

            let (callback_sender, callback_receiver) = mpsc::channel(100);

            let (mut software_stack, software_stack_event_sender) =
                SoftwareStack::<usize, Event<usize>>::new(
                    id,
                    event_sender.clone(),
                    callback_sender,
                    callback_receiver,
                );
            // TODO: can the creation of links belong directly to the ffl abstraction
            let (link_sender, link_receiver) = local_process_factory.new_local_process_link(id);
            let ffl = FairLossPointToPointLinks::new(id, link_sender, link_receiver);
            software_stack.add_module(Box::new(ffl));
            RecoverableRunnable::new(software_stack_event_sender, software_stack)
        },
    );

    let mut id: ID = ID::new();
    let link_factory = Arc::new(Mutex::new(LocalProcessFactory::<usize, Message>::new()));

    let id_1 = id.get();
    let (mut process_1, process_1_sender) = CrashRecovery::new(id_1, link_factory.clone());

    let id_2 = id.get();
    let (mut process_2, _) = CrashRecovery::new(id_2, link_factory.clone());

    let cancel_token = CancellationToken::new();

    tokio::spawn({
        let cancel_token = cancel_token.clone();
        let runnable_creator = runnable_creator.clone();
        async move {
            process_1.run(runnable_creator, cancel_token).await;
        }
    });

    tokio::spawn({
        let cancel_token = cancel_token.clone();
        let runnable_creator = runnable_creator.clone();
        async move {
            process_2.run(runnable_creator, cancel_token).await;
        }
    });

    let init_event = Event {
        component: Component::FairLossPointToPointLinks,
        event_type: EventType::Send,
        process: Some(id_2),
        message: Message::Job(JobType::Job),
    };

    process_1_sender.emit_request_event(init_event).await;

    let cancel_token = CancellationToken::new();
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
//
//
//
//
// add comments
// dry run with local processors -> try with 3 and send separate messages
// *logging: how do I want to structure actions so that's easy to understand
// process abstractions on top the software_stack
//
//
//
//
// TODO: clean up code. Add comments and descriptions, specify which abstractions to implement to
// demonstrate the simple framework works.
// fix cancellations
