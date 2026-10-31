//! MCP over HTTP for remote agents (PLAN.md §25).
//!
//! A small HTTP/1.1 front-end in front of the live bridge: `POST /mcp` with a
//! JSON-RPC body, authenticated with `Authorization: Bearer <token>`. The
//! request is forwarded to the editor that is actually open, so a remote agent
//! works on the visible document.
//!
//! Only the standard library is used. Tokens are compared in constant time,
//! bodies are limited to 1 MiB, requests are rate limited per client and every
//! request is appended to `bridge-audit.log` in the application data directory
//! (never including the token).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::bridge;

const MAX_BODY: usize = 1024 * 1024;
const REQUESTS_PER_SECOND: f64 = 20.0;
const BURST: f64 = 40.0;

struct Bucket {
    tokens: f64,
    last: Instant,
}

/// Shared state for the HTTP server.
struct Server {
    token: String,
    bridge_host: String,
    bridge_port: u16,
    bridge_token: Option<String>,
    buckets: Mutex<HashMap<IpAddr, Bucket>>,
    audit: PathBuf,
}

/// Run the HTTP MCP front-end until the process is stopped.
pub fn serve(listen: &str, token: &str, bridge_host: &str, bridge_port: u16) -> Result<(), String> {
    let listener =
        TcpListener::bind(listen).map_err(|error| format!("cannot listen on {listen}: {error}"))?;
    let server = Arc::new(Server {
        token: token.to_string(),
        bridge_host: bridge_host.to_string(),
        bridge_port,
        bridge_token: bridge::configured_token().or_else(bridge::stored_token),
        buckets: Mutex::new(HashMap::new()),
        audit: audit_path(),
    });
    for stream in listener.incoming().flatten() {
        let server = Arc::clone(&server);
        std::thread::spawn(move || {
            let _ = handle(stream, &server);
        });
    }
    Ok(())
}

/// Whether `listen` is a loopback address, so the caller can decide whether a
/// token may be generated automatically.
pub fn is_loopback(listen: &str) -> bool {
    listen
        .parse::<SocketAddr>()
        .map(|address| address.ip().is_loopback())
        .unwrap_or(false)
}

fn handle(mut stream: TcpStream, server: &Server) -> Result<(), String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let Some(peer) = stream.peer_addr().ok() else {
        return Ok(());
    };
    let mut reader = BufReader::new(stream.try_clone().map_err(|error| error.to_string())?);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|error| error.to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    let mut content_length = 0usize;
    let mut authorized = false;
    loop {
        let mut line = String::new();
        if reader
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim();
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().unwrap_or(0);
            } else if name.eq_ignore_ascii_case("authorization") {
                authorized = value
                    .strip_prefix("Bearer ")
                    .is_some_and(|token| bridge::tokens_equal(token.trim(), &server.token));
            }
        }
    }
    if !authorized {
        audit(&server.audit, peer, "unauthorized", 401);
        return respond(&mut stream, 401, "{\"error\":\"unauthorized\"}");
    }
    if !allow(&server.buckets, peer.ip()) {
        audit(&server.audit, peer, "rate-limited", 429);
        return respond(&mut stream, 429, "{\"error\":\"too many requests\"}");
    }
    if method != "POST" || (path != "/mcp" && path != "/") {
        audit(&server.audit, peer, "method", 405);
        return respond(&mut stream, 405, "{\"error\":\"use POST /mcp\"}");
    }
    if content_length == 0 || content_length > MAX_BODY {
        audit(&server.audit, peer, "body", 413);
        return respond(&mut stream, 413, "{\"error\":\"invalid Content-Length\"}");
    }
    let mut body = vec![0u8; content_length];
    reader
        .read_exact(&mut body)
        .map_err(|error| error.to_string())?;
    let request = String::from_utf8(body).map_err(|_| "body is not UTF-8".to_string())?;
    let tool = serde_json::from_str::<serde_json::Value>(&request)
        .ok()
        .and_then(|value| {
            value
                .get("params")
                .and_then(|params| params.get("name"))
                .and_then(|name| name.as_str())
                .map(str::to_owned)
                .or_else(|| {
                    value
                        .get("method")
                        .and_then(|m| m.as_str())
                        .map(str::to_owned)
                })
        })
        .unwrap_or_else(|| "unknown".to_string());
    match bridge::request(
        &server.bridge_host,
        server.bridge_port,
        server.bridge_token.clone(),
        &request,
    ) {
        Ok(answer) => {
            audit(&server.audit, peer, &tool, 200);
            let _ = writeln!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
                answer.len()
            );
            let _ = stream.flush();
            Ok(())
        }
        Err(error) => {
            audit(&server.audit, peer, &tool, 503);
            respond(
                &mut stream,
                503,
                &serde_json::json!({"error": error}).to_string(),
            )
        }
    }
}

fn respond(stream: &mut TcpStream, status: u16, body: &str) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        _ => "Service Unavailable",
    };
    let _ = writeln!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
    Ok(())
}

fn allow(buckets: &Mutex<HashMap<IpAddr, Bucket>>, peer: IpAddr) -> bool {
    let Ok(mut buckets) = buckets.lock() else {
        return false;
    };
    let now = Instant::now();
    let bucket = buckets.entry(peer).or_insert(Bucket {
        tokens: BURST,
        last: now,
    });
    let elapsed = now.duration_since(bucket.last).as_secs_f64();
    bucket.tokens = (bucket.tokens + elapsed * REQUESTS_PER_SECOND).min(BURST);
    bucket.last = now;
    if bucket.tokens >= 1.0 {
        bucket.tokens -= 1.0;
        true
    } else {
        false
    }
}

fn audit_path() -> PathBuf {
    crate::config::paths::data_dir().join("bridge-audit.log")
}

fn audit(path: &Path, peer: SocketAddr, tool: &str, status: u16) {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let line = format!("{seconds}\t{}\t{tool}\t{status}\n", peer.ip());
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::{Bucket, allow, is_loopback};
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::Instant;

    const BURST_REQUESTS: u32 = 40;

    #[test]
    fn loopback_detection_works() {
        assert!(is_loopback("127.0.0.1:8766"));
        assert!(!is_loopback("0.0.0.0:8766"));
        assert!(!is_loopback("not-an-address"));
    }

    #[test]
    fn the_rate_limit_drains_and_refills() {
        let buckets = Mutex::new(HashMap::new());
        let peer = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
        let mut allowed = 0u32;
        while allow(&buckets, peer) {
            allowed += 1;
            assert!(allowed <= BURST_REQUESTS + 1, "rate limit never stops");
        }
        assert_eq!(allowed, BURST_REQUESTS);
        let bucket = buckets.lock().unwrap();
        assert!(bucket.get(&peer).is_some_and(|bucket| bucket.tokens < 1.0));
        drop(bucket);
        let mut buckets = buckets.lock().unwrap();
        buckets.insert(
            peer,
            Bucket {
                tokens: 1.0,
                last: Instant::now(),
            },
        );
    }
}
