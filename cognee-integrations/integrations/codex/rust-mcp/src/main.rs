mod arguments;
mod client;
mod markdown;
mod mcp;
mod settings;
mod tools;
mod uploads;

use anyhow::Result;
use clap::{Parser, Subcommand};
use client::CogneeClient;
use settings::Settings;

#[derive(Parser)]
#[command(name = "cognee-mcp-rs")]
struct Cli {
    #[arg(long)]
    service_url: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Mcp,
    Describe,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = client_from_cli(&cli)?;
    run_command(&client, cli.command)
}

fn client_from_cli(cli: &Cli) -> Result<CogneeClient> {
    CogneeClient::new(Settings::from_environment(cli.service_url.clone()))
}

fn run_command(client: &CogneeClient, command: Option<Command>) -> Result<()> {
    match command.unwrap_or(Command::Mcp) {
        Command::Mcp => mcp::run_mcp(client.clone()),
        Command::Describe => print_describe(client),
    }
}

fn print_describe(client: &CogneeClient) -> Result<()> {
    println!(
        "{}",
        tools::call_tool(client, "describe", serde_json::Value::Null)?
    );
    Ok(())
}
