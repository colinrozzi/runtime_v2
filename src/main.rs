use anyhow::Result;
use clap::Parser;
use runtime_v2::ActorRuntime;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the actor manifest file
    #[arg(short, long)]
    manifest: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("Starting actor runtime...");
    let (mut runtime, _tx) = ActorRuntime::from_file(args.manifest).await?;

    let runtime_handle = tokio::spawn(async move {
        if let Err(e) = runtime.run().await {
            eprintln!("Runtime error: {}", e);
        }
    });

    println!("Actor is running. Press Ctrl+C to exit.");
    tokio::signal::ctrl_c().await?;

    println!("Shutting down...");
    runtime_handle.abort();
    
    Ok(())
}