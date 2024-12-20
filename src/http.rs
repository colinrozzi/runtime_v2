use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use tide::{Request, Response, Server};
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::{ActorInput, ActorMessage, HostHandler};

// HTTP interface for actor-to-actor communication
#[derive(Clone)]
#[allow(dead_code)]
pub struct HttpHost {
    client: Client,
    port: u16,
    mailbox_tx: mpsc::Sender<ActorMessage>,
}

impl HttpHost {
    pub fn new(mailbox_tx: mpsc::Sender<ActorMessage>) -> Self {
        Self {
            client: Client::new(),
            port: 0, // Default port
            mailbox_tx,
        }
    }

    // Send a message to another actor
    pub async fn send_message(&self, address: String, message: Value) -> Result<()> {
        info!("[HTTP] Sending message to {}", address);
        // Fire and forget POST request
        self.client
            .post(address)
            .json(&message)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send message: {}", e))?;

        Ok(())
    }

    // Handle incoming message
    async fn handle_request(mut req: Request<mpsc::Sender<ActorMessage>>) -> tide::Result {
        match req.method() {
            tide::http::Method::Post => {
                // Get JSON payload
                let payload: Value = req.body_json().await?;

                info!("[HTTP] Received message");

                // Create message with no response channel
                let msg = ActorMessage {
                    content: ActorInput::Message(payload),
                    metadata: None,
                };

                // Send to actor
                req.state()
                    .send(msg)
                    .await
                    .map_err(|_| tide::Error::from_str(500, "Failed to forward message"))?;

                // Simple OK response
                Ok(Response::new(200))
            }
            _ => Ok(Response::new(405)), // Method Not Allowed
        }
    }
}

pub struct HttpHandler {
    port: u16,
}

impl HttpHandler {
    pub fn new(port: u16) -> Self {
        Self { port }
    }
}

impl HostHandler for HttpHandler {
    fn name(&self) -> &str {
        "http"
    }

    fn new(config: Value) -> Self {
        let port = config.get("port").unwrap().as_u64().unwrap() as u16;
        Self { port }
    }

    fn start(
        &self,
        mailbox_tx: mpsc::Sender<ActorMessage>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            let mut app = Server::with_state(mailbox_tx);
            app.at("/").post(HttpHost::handle_request);

            // Spawn the server in a separate task
            let server_port = self.port;
            tokio::spawn(async move {
                match app.listen(format!("127.0.0.1:{}", server_port)).await {
                    Ok(_) => {
                        info!("[HTTP] HTTP server exited");
                    }
                    Err(e) => {
                        error!("[HTTP] HTTP server failed: {}", e);
                    }
                }
            });

            info!("[HTTP] HTTP server started");

            // Keep this task alive
            std::future::pending::<()>().await;

            Ok(())
        })
    }

    fn stop(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        Box::pin(async move {
            // Basic shutdown - the server will stop when dropped
            Ok(())
        })
    }
}
