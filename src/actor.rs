use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;

pub type State = Value;
pub type ActorOutput = Value;

/// The content of an event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// The type of event that occurred
    pub type_: String,
    /// The data associated with this event
    pub data: Value,
}

impl Event {
    pub fn noop() -> Self {
        Self {
            type_: "noop".to_string(),
            data: Value::Null,
        }
    }
}

pub trait Actor: Send {
    async fn init(&self) -> Result<Value>;
    async fn handle_event(&self, state: State, event: Event) -> Result<(State, Event)>;
    async fn verify_state(&self, state: &Value) -> bool;
}