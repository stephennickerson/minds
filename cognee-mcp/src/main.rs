mod arguments;
mod client;
mod markdown;
mod mcp;
mod read_model;
mod settings;
mod tools;
mod uploads;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use client::CogneeClient;
use settings::Settings;
use std::io::{self, Read};

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
    Tool(ToolCommand),
}

#[derive(Args)]
struct ToolCommand {
    name: String,
    arguments: Option<String>,
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
        Command::Tool(command) => print_tool(client, command),
    }
}

fn print_describe(client: &CogneeClient) -> Result<()> {
    println!(
        "{}",
        tools::call_tool(client, "describe", serde_json::Value::Null)?
    );
    Ok(())
}

fn print_tool(client: &CogneeClient, command: ToolCommand) -> Result<()> {
    let raw_arguments = tool_arguments(command.arguments)?;
    let arguments = serde_json::from_str(&raw_arguments)?;
    println!("{}", tools::call_tool(client, &command.name, arguments)?);
    Ok(())
}

fn tool_arguments(arguments: Option<String>) -> Result<String> {
    match arguments.as_deref() {
        Some("-") => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            Ok(buffer)
        }
        Some(value) => Ok(value.to_string()),
        None => Ok("{}".to_string()),
    }
}
