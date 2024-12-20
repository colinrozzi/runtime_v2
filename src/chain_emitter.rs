use std::collections::VecDeque;
use std::sync::Mutex;
use chrono::Utc;
use tokio::sync::broadcast;
use serde::Serialize;

use crate::logging::ChainEvent;

pub struct ChainEmitter {
    history: Mutex<VecDeque<ChainEvent>>,
    max_history: usize,
    tx: broadcast::Sender<ChainEvent>,
}

impl ChainEmitter {
    pub fn new(max_history: usize) -> Self {
        let (tx, _) = broadcast::channel(1000); // Buffer size of 1000 events
        Self {
            history: Mutex::new(VecDeque::with_capacity(max_history)),
            max_history,
            tx,
        }
    }

    pub fn emit(&self, event: ChainEvent) {
        // Store in history
        let mut history = self.history.lock().unwrap();
        if history.len() >= self.max_history {
            history.pop_front();
        }
        history.push_back(event.clone());

        // Broadcast to all subscribers
        let _ = self.tx.send(event.clone());

        // Print to stdout in a clearly marked format
        println!("\n[CHAIN] Event at {}", Utc::now().to_rfc3339());
        println!("{}", event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ChainEvent> {
        self.tx.subscribe()
    }

    pub fn get_history(&self) -> Vec<ChainEvent> {
        self.history.lock().unwrap().iter().cloned().collect()
    }
}

// Global instance
lazy_static::lazy_static! {
    pub static ref CHAIN_EMITTER: ChainEmitter = ChainEmitter::new(1000);
}