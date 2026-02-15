#![recursion_limit = "256"]

mod app;
mod audio;
mod command;
mod event;
mod fx;
mod mcp;
mod project;
mod sequencer;
mod synth;
mod ui;

use anyhow::Result;
use clap::Parser;

use app::App;
use mcp::run_as_proxy;
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

    // MCP server mode — requires TUI to be running (connects via socket)
    if args.mcp {
        if let Err(e) = run_as_proxy() {
            // Write a JSON-RPC error to stdout so MCP clients see a clear message
            let msg = format!(
                "gridoxide TUI is not running. Start it first with: gridoxide ({})", e
            );
            let err_response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32000,
                    "message": msg
                }
            });
            println!("{}", err_response);
            return Err(anyhow::anyhow!("{}", msg));
        }
        return Ok(());
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
