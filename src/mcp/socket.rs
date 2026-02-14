use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::GridoxideMcp;

pub const SOCKET_PATH: &str = "/tmp/gridoxide.sock";

/// Handle a single JSON-RPC request line, return response (or None for notifications)
fn handle_jsonrpc_line(line: &str, mcp: &GridoxideMcp) -> Option<String> {
    let request: serde_json::Value = match serde_json::from_str(line) {
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
            return Some(error_response.to_string());
        }
    };

    let id = request.get("id").cloned();
    let method = request
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("");
    let params = request
        .get("params")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let result = match method {
        "initialize" => {
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "gridoxide",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })
        }
        "tools/list" => GridoxideMcp::list_tools(),
        "tools/call" => {
            let tool_name = params
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let tool_result = mcp.handle_tool_call(tool_name, &arguments);
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&tool_result).unwrap_or_default()
                }]
            })
        }
        "notifications/initialized" => return None,
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

    Some(response.to_string())
}

/// Handle a single client connection on the socket
fn handle_connection(stream: UnixStream, mcp: &GridoxideMcp) {
    let reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut writer = stream;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.is_empty() {
            continue;
        }

        if let Some(response) = handle_jsonrpc_line(&line, mcp) {
            if writeln!(writer, "{}", response).is_err() {
                break;
            }
            if writer.flush().is_err() {
                break;
            }
        }
    }
}

/// Start the MCP socket server in a background thread.
/// Shares the same command bus and state as the TUI.
pub fn start_socket_server(mcp: Arc<GridoxideMcp>, shutdown: Arc<AtomicBool>) {
    // Remove stale socket file
    let _ = std::fs::remove_file(SOCKET_PATH);

    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(_) => return,
    };

    // Non-blocking so we can check the shutdown flag periodically
    listener.set_nonblocking(true).ok();

    std::thread::spawn(move || {
        while !shutdown.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).ok();
                    let mcp = mcp.clone();
                    std::thread::spawn(move || handle_connection(stream, &mcp));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
        // Clean up socket file on shutdown
        let _ = std::fs::remove_file(SOCKET_PATH);
    });
}

/// Run as a stdio-to-socket proxy.
/// Forwards JSON-RPC from stdin to the TUI's socket, responses back to stdout.
/// Returns Ok(()) on success, Err if the socket is not available.
pub fn run_as_proxy() -> Result<(), std::io::Error> {
    let stream = UnixStream::connect(SOCKET_PATH)?;
    let mut socket_reader = BufReader::new(stream.try_clone()?);
    let mut socket_writer = stream;

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.is_empty() {
            continue;
        }

        // Forward request to socket
        writeln!(socket_writer, "{}", line)?;
        socket_writer.flush()?;

        // Check if this is a notification (no response expected)
        if let Ok(req) = serde_json::from_str::<serde_json::Value>(&line) {
            let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
            if method.starts_with("notifications/") {
                continue;
            }
        }

        // Read response from socket and forward to stdout
        let mut response = String::new();
        socket_reader.read_line(&mut response)?;
        write!(stdout, "{}", response)?;
        stdout.flush()?;
    }

    Ok(())
}
