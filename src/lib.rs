use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

mod config;
mod http;
mod wasm;

pub use config::ActorConfig;

#[derive(Debug)]
pub enum ActorMessage {
    Regular {
        content: Value,
        response: Option<oneshot::Sender<Value>>,
    },
    Http {
        method: String,
        uri: String,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
        response: oneshot::Sender<(u16, Vec<(String, String)>, Option<Vec<u8>>)>,
    },
}

pub struct ActorRuntime {
    actor: Arc<wasm::WasmActor>,
    state: Value,
    mailbox: mpsc::Receiver<ActorMessage>,
    http_server: Option<tokio::task::JoinHandle<()>>,
}

impl ActorRuntime {
    pub async fn from_file(manifest_path: PathBuf) -> Result<(Self, mpsc::Sender<ActorMessage>)> {
        let config = ActorConfig::from_file(&manifest_path)?;
        let actor = Arc::new(wasm::WasmActor::from_file(manifest_path)?);
        
        let (tx, rx) = mpsc::channel(32);
        let state = actor.init()?;

        // Start HTTP server if configured
        let http_server = if let Some(port) = config.http_port {
            let server_actor = actor.clone();
            let server_tx = tx.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = http::serve(port, server_actor, server_tx).await {
                    eprintln!("HTTP server error: {}", e);
                }
            });
            Some(handle)
        } else {
            None
        };

        Ok((Self {
            actor,
            state,
            mailbox: rx,
            http_server,
        }, tx))
    }

    pub async fn run(&mut self) -> Result<()> {
        while let Some(msg) = self.mailbox.recv().await {
            match msg {
                ActorMessage::Regular { content, response } => {
                    let (output, new_state) = self.actor.handle_message(&content, &self.state)?;
                    self.state = new_state;
                    if let Some(tx) = response {
                        let _ = tx.send(output);
                    }
                }
                ActorMessage::Http { method, uri, headers, body, response } => {
                    let (status, resp_headers, resp_body, new_state) = 
                        self.actor.handle_http(method, uri, headers, body, &self.state)?;
                    self.state = new_state;
                    let _ = response.send((status, resp_headers, resp_body));
                }
            }
        }
        Ok(())
    }

    pub async fn shutdown(&mut self) {
        if let Some(server) = self.http_server.take() {
            server.abort();
        }
    }
}