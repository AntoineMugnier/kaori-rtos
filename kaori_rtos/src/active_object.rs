use kaori_hsm::{StateMachine, TopState};

struct ActiveObject<UserStateMachine: TopState>{
    pub(crate) evt_queue : EvtQueue<<UserStateMachine as TopState>::Evt>,
    pub(crate) state_machine: StateMachine<UserStateMachine>
}
