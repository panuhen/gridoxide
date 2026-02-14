mod app;
mod audio;
mod command;
mod event;
mod mcp;
mod sequencer;
mod synth;
mod ui;

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use parking_lot::RwLock;

use app::App;
use audio::AudioEngine;
use command::CommandBus;
use event::EventLog;
use mcp::{run_as_proxy, GridoxideMcp};
use ui::Theme;

/// Gridoxide - Terminal EDM Production Studio
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Theme to use for the interface
    #[arg(long, default_value = "default")]
    theme: String,

    /// List available themes and exit
    #[arg(long)]
    list_themes: bool,

    /// Run in MCP server mode (JSON-RPC over stdio)
    #[arg(long)]
    mcp: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Handle --list-themes
    if args.list_themes {
        println!("Available themes:");
        for theme in Theme::available_themes() {
            println!("  {}", theme);
        }
        return Ok(());
    }

    // MCP server mode
    if args.mcp {
        // Try connecting to the TUI's socket first (shared state)
        if run_as_proxy().is_ok() {
            return Ok(());
        }
        // Fall back to standalone MCP server (own audio engine)
        return run_mcp_server();
    }

    // Load theme
    let theme = Theme::from_name(&args.theme).unwrap_or_else(|| {
        eprintln!(
            "Warning: Unknown theme '{}', using default. Use --list-themes to see available themes.",
            args.theme
        );
        Theme::default()
    });

    // Run the TUI application
    let mut app = App::new(theme)?;
    app.run()
}

/// Run gridoxide as a standalone MCP server (headless, JSON-RPC over stdio)
fn run_mcp_server() -> Result<()> {
    // Create command bus
    let command_bus = CommandBus::new();
    let command_sender = command_bus.sender();
    let command_receiver = command_bus.receiver();

    // Create audio engine
    let audio = AudioEngine::new(command_receiver)?;
    let sequencer_state = audio.state.clone();

    // Create event log
    let event_log = Arc::new(RwLock::new(EventLog::new()));

    // Create MCP handler
    let mcp = GridoxideMcp::new(command_sender, event_log, sequencer_state);

    // Keep audio engine alive
    let _audio = audio;

    // Simple JSON-RPC over stdio
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }

        // Parse JSON-RPC request
        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let error_response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {}", e)
                    }
                });
                writeln!(stdout, "{}", error_response)?;
                stdout.flush()?;
                continue;
            }
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(serde_json::json!({}));

        let result = match method {
            "initialize" => {
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "gridoxide",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                })
            }
            "tools/list" => GridoxideMcp::list_tools(),
            "tools/call" => {
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));
                let tool_result = mcp.handle_tool_call(tool_name, &arguments);
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&tool_result)?
                    }]
                })
            }
            "notifications/initialized" => {
                // No response needed for notifications
                continue;
            }
            _ => {
                serde_json::json!({
                    "error": {
                        "code": -32601,
                        "message": format!("Method not found: {}", method)
                    }
                })
            }
        };

        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        });

        writeln!(stdout, "{}", response)?;
        stdout.flush()?;
    }

    Ok(())
}
