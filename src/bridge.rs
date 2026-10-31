//! Live bridge to the running editor (PLAN.md §25).
//!
//! The GUI listens on `127.0.0.1` and executes JSON-RPC requests on the UI
//! thread, against the document that is actually open. `BeamWright mcp --live`
//! proxies MCP stdio to this bridge, so an agent controls the visible session
//! instead of a separate copy.
//!
//! The socket is local-only: nothing is exposed outside the machine, no port is
//! opened to the network and no data leaves the computer.

use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender, channel};

/// Port used when `BEAMWRIGHT_BRIDGE_PORT` is not set.
pub const DEFAULT_PORT: u16 = 47_821;

/// Environment variable that overrides the port.
pub const PORT_ENV: &str = "BEAMWRIGHT_BRIDGE_PORT";

/// Address to bind. Defaults to loopback; set to `0.0.0.0` or a Tailscale
/// address to accept connections from other machines.
pub const ADDRESS_ENV: &str = "BEAMWRIGHT_BRIDGE_ADDR";

/// Shared secret required for non-loopback connections.
pub const TOKEN_ENV: &str = "BEAMWRIGHT_BRIDGE_TOKEN";

/// One request read from a client, with the channel used to answer it.
pub struct BridgeRequest {
    pub request: String,
    pub reply: Sender<String>,
}

/// Running bridge: owns the listener thread and the request queue.
pub struct LiveBridge {
    port: u16,
    address: IpAddr,
    requests: Receiver<BridgeRequest>,
}

impl LiveBridge {
    /// Start the bridge on `port`, or on the next free port when it is taken.
    ///
    /// Returns `None` when no local port can be bound; the editor keeps working.
    pub fn start(address: IpAddr, port: u16, token: Option<String>) -> Option<Self> {
        let mut address = address;
        let token = token.filter(|value| !value.trim().is_empty());
        if !address.is_loopback() && token.is_none() {
            // Never expose an unauthenticated editing socket to the network.
            address = IpAddr::V4(Ipv4Addr::LOCALHOST);
        }
        let listener = bind(address, port)?;
        let (sender, requests) = channel();
        let actual_port = listener.local_addr().ok()?.port();
        std::thread::spawn(move || accept_loop(listener, sender, token));
        Some(Self {
            port: actual_port,
            address,
            requests,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn address(&self) -> IpAddr {
        self.address
    }

    pub fn is_network(&self) -> bool {
        !self.address.is_loopback()
    }

    /// Take every request waiting in the queue.
    pub fn drain(&self) -> Vec<BridgeRequest> {
        let mut drained = Vec::new();
        while let Ok(request) = self.requests.try_recv() {
            drained.push(request);
        }
        drained
    }
}

fn bind(address: IpAddr, preferred: u16) -> Option<TcpListener> {
    for candidate in preferred..preferred.saturating_add(16) {
        if let Ok(listener) = TcpListener::bind((address, candidate)) {
            return Some(listener);
        }
    }
    None
}

#[allow(clippy::needless_pass_by_value)]
fn accept_loop(listener: TcpListener, sender: Sender<BridgeRequest>, token: Option<String>) {
    for stream in listener.incoming().flatten() {
        let sender = sender.clone();
        let token = token.clone();
        std::thread::spawn(move || serve_client(stream, sender, token));
    }
}

#[allow(clippy::needless_pass_by_value)]
fn serve_client(stream: TcpStream, sender: Sender<BridgeRequest>, token: Option<String>) {
    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let reader = BufReader::new(reader_stream);
    let mut writer = stream;
    let mut lines = reader.lines().map_while(Result::ok);
    if let Some(expected) = &token {
        let Some(first) = lines.next() else {
            return;
        };
        let authorized = serde_json::from_str::<serde_json::Value>(&first)
            .ok()
            .and_then(|value| {
                value
                    .get("token")
                    .and_then(|token| token.as_str())
                    .map(str::to_owned)
            })
            .is_some_and(|value| value == *expected);
        if !authorized {
            let _ = writeln!(
                writer,
                "{{\"jsonrpc\":\"2.0\",\"error\":{{\"code\":-32001,\"message\":\"unauthorized\"}}}}"
            );
            let _ = writer.flush();
            return;
        }
        let _ = writeln!(writer, "{{\"ok\":true}}");
        let _ = writer.flush();
    }
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let (reply, answer) = channel();
        if sender
            .send(BridgeRequest {
                request: line,
                reply,
            })
            .is_err()
        {
            return;
        }
        let Ok(response) = answer.recv() else {
            return;
        };
        if writeln!(writer, "{response}").is_err() {
            return;
        }
        let _ = writer.flush();
    }
}

/// Proxy MCP stdio to a running editor.
///
/// Lines from stdin are forwarded to the bridge and the answers are printed to
/// stdout unchanged, so an MCP client sees the usual JSON-RPC stream.
pub fn proxy_stdio(host: &str, port: u16, token: Option<String>) -> Result<(), String> {
    let stream = TcpStream::connect((host, port)).map_err(|error| {
        format!("no running BeamWright editor on {host}:{port} ({error}); start BeamWright first")
    })?;
    if let Some(token) = token {
        authenticate(
            stream.try_clone().map_err(|error| error.to_string())?,
            &token,
        )?;
    }
    let mut writer = stream
        .try_clone()
        .map_err(|error| format!("cannot use the bridge connection: {error}"))?;
    let reader = BufReader::new(stream);
    let stdin_thread = std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines().map_while(Result::ok) {
            if writeln!(writer, "{line}").is_err() {
                return;
            }
            let _ = writer.flush();
        }
    });
    for line in reader.lines().map_while(Result::ok) {
        println!("{line}");
        let _ = std::io::stdout().flush();
    }
    let _ = stdin_thread.join();
    Ok(())
}

/// Send one JSON-RPC request to a running editor and print the answer.
pub fn request(
    host: &str,
    port: u16,
    token: Option<String>,
    request: &str,
) -> Result<String, String> {
    let mut stream = TcpStream::connect((host, port)).map_err(|error| {
        format!("no running BeamWright editor on {host}:{port} ({error}); start BeamWright first")
    })?;
    if let Some(token) = token {
        authenticate(
            stream.try_clone().map_err(|error| error.to_string())?,
            &token,
        )?;
    }
    writeln!(stream, "{}", request.trim())
        .map_err(|error| format!("cannot send the request: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("cannot send the request: {error}"))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|error| format!("cannot read the answer: {error}"))?;
    if line.trim().is_empty() {
        return Err("the editor closed the connection without answering".to_string());
    }
    Ok(line.trim().to_string())
}

/// Read the token the editor requires for non-loopback connections.
pub fn configured_token() -> Option<String> {
    std::env::var(TOKEN_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// Address from the environment, or loopback.
pub fn configured_address() -> IpAddr {
    std::env::var(ADDRESS_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

fn authenticate(mut stream: TcpStream, token: &str) -> Result<(), String> {
    let handshake = serde_json::json!({"token": token});
    writeln!(stream, "{handshake}").map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|error| error.to_string())?;
    if line.contains("\"ok\":true") {
        Ok(())
    } else {
        Err("the editor rejected the bridge token".to_string())
    }
}

/// Port from the environment, or the default.
pub fn configured_port() -> u16 {
    std::env::var(PORT_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

#[cfg(test)]
mod tests {
    use super::configured_port;

    #[test]
    fn the_default_port_is_stable() {
        let port = configured_port();
        assert!(port > 1024);
    }
}
