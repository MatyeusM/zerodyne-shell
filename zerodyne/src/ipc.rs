//! The local JSON protocol spoken over the daemon's Unix socket. One
//! connection carries exactly one request and one response, each a single
//! JSON object terminated by `\n`. Transport only — dispatch is in `daemon`.
//!
//! ```text
//! {"op":"status","target":"network"}
//! {"op":"status"}
//! {"op":"exec","target":"wifi-band","args":["5"]}
//! {"op":"ping"}
//! {"op":"stop"}
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

pub const OP_STATUS: &str = "status";
pub const OP_EXEC: &str = "exec";
pub const OP_PING: &str = "ping";
pub const OP_STOP: &str = "stop";

pub const DIAL_TIMEOUT: Duration = Duration::from_secs(2);
pub const IO_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_LINE: u64 = 256 * 1024;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct Request {
    #[serde(default)]
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    // Missing `args` must decode as empty: QML/CLI only send it for exec.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
}

impl Response {
    /// Build a success response from any serializable payload.
    pub fn ok(value: impl Serialize) -> Self {
        match serde_json::to_value(value) {
            Ok(mut data) => {
                normalize_floats(&mut data);
                Response {
                    ok: true,
                    data: Some(data),
                    error: String::new(),
                }
            }
            Err(e) => Self::error(format!("encode response: {e}")),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Response {
            ok: false,
            data: None,
            error: message.into(),
        }
    }
}

/// Go's `encoding/json` writes whole float64s without a decimal point
/// (`62`, not `62.0`). Normalise here so the CLI/JSON on the wire matches
/// the Go daemon byte-for-byte for numeric fields.
fn normalize_floats(v: &mut Value) {
    match v {
        Value::Number(n) => {
            if let Some(f) = n.as_f64()
                && f.fract() == 0.0
                && f >= i64::MIN as f64
                && f <= i64::MAX as f64
            {
                *v = Value::Number(serde_json::Number::from(f as i64));
            }
        }
        Value::Array(items) => items.iter_mut().for_each(normalize_floats),
        Value::Object(map) => map.values_mut().for_each(normalize_floats),
        _ => {}
    }
}

/// Dial `sock_path`, write one request, read one response.
pub fn send(sock_path: &Path, req: &Request) -> Result<Response> {
    let mut conn =
        UnixStream::connect(sock_path).with_context(|| format!("dial {}", sock_path.display()))?;
    conn.set_read_timeout(Some(IO_TIMEOUT))?;
    conn.set_write_timeout(Some(IO_TIMEOUT))?;

    let mut raw = serde_json::to_vec(req)?;
    raw.push(b'\n');
    conn.write_all(&raw)?;
    conn.flush()?;

    let mut line = Vec::new();
    let mut limited = BufReader::new((&mut conn).take(MAX_LINE + 1));
    limited.read_until(b'\n', &mut line)?;
    if !line.ends_with(b"\n") {
        anyhow::bail!("response missing newline");
    }
    serde_json::from_slice(&line).context("decode response")
}

/// Read one newline-terminated request from a server-side connection.
pub fn read_request(conn: &mut UnixStream) -> Result<Request> {
    conn.set_read_timeout(Some(IO_TIMEOUT))?;
    conn.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut line = Vec::new();
    let mut limited = BufReader::new(conn.take(MAX_LINE + 1));
    limited.read_until(b'\n', &mut line)?;
    if !line.ends_with(b"\n") {
        anyhow::bail!("request missing newline");
    }
    serde_json::from_slice(&line).context("decode request")
}

/// Write one newline-terminated response to a server-side connection.
pub fn write_response(conn: &mut UnixStream, resp: &Response) -> Result<()> {
    conn.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut raw = serde_json::to_vec(resp)?;
    raw.push(b'\n');
    conn.write_all(&raw)?;
    conn.flush()?;
    Ok(())
}

/// Liveness probe: true only when a daemon answers OK at `sock_path`.
pub fn ping(sock_path: &Path) -> bool {
    match send(
        sock_path,
        &Request {
            op: OP_PING.to_string(),
            ..Default::default()
        },
    ) {
        Ok(resp) => resp.ok,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::thread;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("zrs-ipc-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let sock = dir.join("s.sock");
        let _ = std::fs::remove_file(&sock);
        let listener = UnixListener::bind(&sock).unwrap();
        let server = thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let req = read_request(&mut conn).unwrap();
            assert_eq!(req.op, OP_STATUS);
            assert_eq!(req.target.as_deref(), Some("network"));
            write_response(&mut conn, &Response::ok(serde_json::json!({"kind":"wifi"}))).unwrap();
        });
        let resp = send(
            &sock,
            &Request {
                op: OP_STATUS.into(),
                target: Some("network".into()),
                args: vec![],
            },
        )
        .unwrap();
        assert!(resp.ok);
        assert_eq!(resp.data.unwrap()["kind"], "wifi");
        server.join().unwrap();
        let _ = std::fs::remove_file(&sock);
    }

    #[test]
    fn dial_error_and_error_response() {
        let missing = std::env::temp_dir().join(format!("zrs-missing-{}", std::process::id()));
        assert!(send(&missing, &Request::default()).is_err());
        assert!(!ping(&missing));
        let resp = Response::error("boom 42");
        assert_eq!(resp.error, "boom 42");
    }
}
