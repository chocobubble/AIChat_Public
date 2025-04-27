mod gemini_client;
mod cli;

use std::io;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use dotenv::dotenv;
use eyre::Result;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use crate::cli::chat::ChatContext;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Input to send to the chat
    #[arg(short, long)]
    input: Option<String>,
    
    /// Accept all prompts without asking
    #[arg(short, long)]
    yes: bool,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a chat session
    Chat {
        /// Input to send to the chat
        #[arg(short, long)]
        input: Option<String>,
        
        /// Enable verbose logging
        #[arg(short, long)]
        verbose: bool,
    },
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    // Load environment variables from .env file
    dotenv().ok();
    
    let cli = Cli::parse();
    
    // Initialize tracing with appropriate level
    let verbose = match &cli.command {
        Some(Commands::Chat { verbose, .. }) => *verbose,
        None => cli.verbose,
    };
    
    let log_level = if verbose { Level::DEBUG } else { Level::INFO };
    
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");
    
    info!("Starting Gemini Chat CLI");
    
    match cli.command {
        Some(Commands::Chat { input, .. }) => {
            let mut chat_context = ChatContext::new(
                Box::new(io::stdout()),
                input,
                true,
                cli.yes,
            );
            chat_context.run().await
        }
        None => {
            // Default to chat if no subcommand is provided
            let mut chat_context = ChatContext::new(
                Box::new(io::stdout()),
                cli.input,
                true,
                cli.yes,
            );
            chat_context.run().await
        }
    }
}
