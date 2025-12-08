use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Event<State> {
    StateChanged(Arc<State>),
    // TODO: get rid of anyhow? This is contorted.
    Error(Arc<anyhow::Error>),
}

pub trait StateMachine {
    type State;
    type Action;

    async fn initiate(&mut self) -> anyhow::Result<()>;
    async fn terminate(&mut self) -> anyhow::Result<()>;
    async fn next_event(&mut self) -> Option<Event<Self::State>>;
    async fn request_state(&mut self);
    async fn apply(&mut self, action: Self::Action) -> anyhow::Result<()>;
}
