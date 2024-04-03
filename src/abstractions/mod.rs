pub mod communication;
pub mod composition_model;
pub mod distributed_system;
pub mod logging;
pub mod process;

#[derive(Clone, Copy, Debug)]
pub struct Event<I>
where
    I: Send,
{
    pub component: Component,
    pub event_type: EventType,
    pub process: Option<I>,
    pub message: Message,
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
    Nil,
    Job(JobType),
    Data(DataType),
    Proposal(ProposalType),
}

#[derive(Clone, Copy, Debug)]
pub enum JobType {
    Job,
}
#[derive(Clone, Copy, Debug)]
pub enum DataType {}

#[derive(Clone, Copy, Debug)]
pub enum ProposalType {
    Flooding { round: usize, proposal_set: usize },
    Hierarchical { value: usize },
}
