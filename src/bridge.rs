use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

pub const DEFAULT_PORT: u16 = 17891;
const MAX_M3U8_SIZE: usize = 16 * 1024 * 1024;
const MAX_STORE_SIZE: usize = 256 * 1024 * 1024;
const MAX_CONCURRENT_CONNECTIONS: usize = 32;
const ENTRY_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
struct Entry {
    content: String,
    last_access: SystemTime,
}

struct StoreData {
    entries: HashMap<String, Entry>,
    total_bytes: usize,
}

type Store = Arc<Mutex<StoreData>>;

static ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

/// Returns true when the ush:// URL explicitly requests the HTTP bridge.
pub fn needs_server(input: &str) -> bool {
    let query = match input.split_once('?') {
        Some((_, query)) => query,
        None => return false,
    };

    query.split('&').any(|part| {
        let mut kv = part.splitn(2, '=');
        let key = kv.next().unwrap_or("");
        let value = kv.next().unwrap_or("");
        key.eq_ignore_ascii_case("needServer") && value == "1"
    })
}

/// Ensure a standalone bridge process is running. The caller does not need to
/// stay alive after this returns.
pub fn ensure_server() -> Result<(), Box<dyn Error + Send + Sync>> {
    if is_server_available() {
        return Ok(());
    }

    // When running as an AppImage, current_exe() points into the temporary
    // AppImage mount. Relaunch the real .AppImage file so the bridge process
    // remains valid after the URL-handler process exits.
    let exe = env::var_os("APPIMAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or(env::current_exe()?);

    Command::new(&exe)
        .arg("--bridge-server")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| -> Box<dyn Error + Send + Sync> {
            format!("failed to start HTTP Bridge: {e}").into()
        })?;

    // spawn() only means that the child process was created. Give it time to
    // bind the listener, but do not turn a slow startup into a GUI error.
    // The userscript independently polls /api/status before uploading the
    // playlist, so the URL handler does not need to block the browser launch.
    //
    // This is especially useful on Windows/macOS when the executable is
    // started from a URL handler: process creation can occasionally take
    // several seconds even though the bridge eventually starts correctly.
    for _ in 0..100 {
        if is_server_available() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    // The child was successfully spawned. It may still be starting, so treat
    // this as a best-effort startup rather than showing an error dialog.
    // The userscript's /api/status polling is the authoritative readiness
    // check.
    eprintln!(
        "scheme-handler: HTTP Bridge is still starting on 127.0.0.1:{DEFAULT_PORT}"
    );
    Ok(())
}

fn is_server_available() -> bool {
    let address = match format!("127.0.0.1:{DEFAULT_PORT}").parse() {
        Ok(address) => address,
        Err(_) => return false,
    };

    let mut stream = match TcpStream::connect_timeout(&address, Duration::from_millis(200)) {
        Ok(stream) => stream,
        Err(_) => return false,
    };

    let request = format!(
        "GET /api/status HTTP/1.1\r\nHost: 127.0.0.1:{DEFAULT_PORT}\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut response = [0u8; 1024];
    let n = match stream.read(&mut response) {
        Ok(n) => n,
        Err(_) => return false,
    };
    let text = String::from_utf8_lossy(&response[..n]);
    text.starts_with("HTTP/1.1 200 OK") && text.contains(r#""ok":true"#)
}

/// Run the standalone HTTP bridge process. This function intentionally blocks.
pub fn run_server() -> Result<(), Box<dyn Error + Send + Sync>> {
    let listener = match TcpListener::bind(("127.0.0.1", DEFAULT_PORT)) {
        Ok(listener) => listener,
        Err(error) => {
            // Another bridge may have won the startup race. In that case this
            // process can simply exit successfully.
            if is_server_available() {
                return Ok(());
            }
            return Err(format!("failed to bind HTTP Bridge on 127.0.0.1:{DEFAULT_PORT}: {error}").into());
        }
    };

    let store: Store = Arc::new(Mutex::new(StoreData {
        entries: HashMap::new(),
        total_bytes: 0,
    }));

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if !try_acquire_connection() {
                    let _ = send_text_error(
                        &mut stream,
                        429,
                        "Too Many Requests",
                        "too many concurrent HTTP Bridge connections",
                    );
                    continue;
                }

                let store = Arc::clone(&store);
                thread::spawn(move || {
                    let _guard = ConnectionGuard;
                    if let Err(error) = handle_connection(stream, store) {
                        eprintln!("scheme-handler HTTP Bridge: {error}");
                    }
                });
            }
            Err(error) => eprintln!("scheme-handler HTTP Bridge accept error: {error}"),
        }
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream, store: Store) -> Result<(), Box<dyn Error + Send + Sync>> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    let (method, path, body) = read_request(&mut stream)?;

    match (method.as_str(), path.as_str()) {
        ("OPTIONS", _) => send_response(
            &mut stream,
            204,
            "No Content",
            "text/plain; charset=utf-8",
            b"",
        )?,
        ("GET", "/api/status") => {
            let body = format!(r#"{{"ok":true,"port":{DEFAULT_PORT}}}"#);
            send_response(
                &mut stream,
                200,
                "OK",
                "application/json; charset=utf-8",
                body.as_bytes(),
            )?;
        }
        ("POST", "/api/m3u8") => {
            let content = String::from_utf8(body)
                .map_err(|_| "m3u8 body is not valid UTF-8")?;
            if content.trim_start_matches('\u{feff}').trim_start().lines().next() != Some("#EXTM3U") {
                return send_text_error(&mut stream, 400, "Bad Request", "request body is not a valid m3u8");
            }

            let id = new_id();
            {
                let mut store = store.lock().map_err(|_| "bridge store lock poisoned")?;
                cleanup_expired(&mut store);

                let content_len = content.len();
                if content_len > MAX_STORE_SIZE {
                    return send_text_error(
                        &mut stream,
                        413,
                        "Payload Too Large",
                        "m3u8 is larger than the Bridge cache limit",
                    );
                }

                make_room(&mut store, content_len);

                store.total_bytes += content_len;
                store.entries.insert(
                    id.clone(),
                    Entry {
                        content,
                        last_access: SystemTime::now(),
                    },
                );
            }

            let body = format!(
                r#"{{"url":"http://127.0.0.1:{DEFAULT_PORT}/m3u8/{id}"}}"#
            );
            send_response(
                &mut stream,
                200,
                "OK",
                "application/json; charset=utf-8",
                body.as_bytes(),
            )?;
        }
        ("GET", path) if path.starts_with("/m3u8/") => {
            let id = &path[6..];
            let content = {
                let mut store = store.lock().map_err(|_| "bridge store lock poisoned")?;
                cleanup_expired(&mut store);
                let entry = match store.entries.get_mut(id) {
                    Some(entry) => entry,
                    None => return send_text_error(&mut stream, 404, "Not Found", "m3u8 not found"),
                };
                entry.last_access = SystemTime::now();
                entry.content.clone()
            };

            send_response(
                &mut stream,
                200,
                "OK",
                "application/vnd.apple.mpegurl; charset=utf-8",
                content.as_bytes(),
            )?;
        }
        _ => send_text_error(&mut stream, 404, "Not Found", "not found")?,
    }

    Ok(())
}

fn cleanup_expired(store: &mut StoreData) {
    let now = SystemTime::now();
    let expired: Vec<String> = store
        .entries
        .iter()
        .filter_map(|(id, entry)| {
            let expired = now
                .duration_since(entry.last_access)
                .map(|age| age > ENTRY_TTL)
                .unwrap_or(false);
            expired.then(|| id.clone())
        })
        .collect();

    for id in expired {
        remove_entry(store, &id);
    }
}

fn make_room(store: &mut StoreData, required_bytes: usize) {
    while store.total_bytes.saturating_add(required_bytes) > MAX_STORE_SIZE {
        let oldest_id = store
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(id, _)| id.clone());

        match oldest_id {
            Some(id) => remove_entry(store, &id),
            None => break,
        }
    }
}

fn remove_entry(store: &mut StoreData, id: &str) {
    if let Some(entry) = store.entries.remove(id) {
        store.total_bytes = store.total_bytes.saturating_sub(entry.content.len());
    }
}

fn new_id() -> String {
    use rand::RngCore;

    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);

    // 128 bits from the OS-backed RNG exposed by rand/getrandom. This ID is
    // intentionally unpredictable because it also acts as the lookup key
    // for the locally hosted playlist.
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn try_acquire_connection() -> bool {
    ACTIVE_CONNECTIONS
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
            (active < MAX_CONCURRENT_CONNECTIONS).then_some(active + 1)
        })
        .is_ok()
}

struct ConnectionGuard;

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::AcqRel);
    }
}

fn read_request(stream: &mut TcpStream) -> Result<(String, String, Vec<u8>), Box<dyn Error + Send + Sync>> {
    let mut headers = Vec::with_capacity(8192);
    let mut buffer = [0u8; 4096];
    let header_end;

    loop {
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            return Err("connection closed before HTTP headers were received".into());
        }
        headers.extend_from_slice(&buffer[..n]);
        if headers.len() > 64 * 1024 {
            return Err("HTTP headers are too large".into());
        }
        if let Some(pos) = find_bytes(&headers, b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }

    let header_text = std::str::from_utf8(&headers[..header_end])?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().ok_or("missing HTTP request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("missing HTTP method")?.to_string();
    let path = parts.next().ok_or("missing HTTP path")?.to_string();

    let mut content_length = 0usize;
    let mut chunked = false;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>()?;
            } else if name.eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
            {
                chunked = true;
            }
        }
    }

    if content_length > MAX_M3U8_SIZE {
        return Err("m3u8 body is too large".into());
    }
    if chunked {
        return Err("chunked request bodies are not supported; send m3u8 with Content-Length".into());
    }

    let mut body = headers[header_end..].to_vec();
    if body.len() > content_length {
        body.truncate(content_length);
    }
    while body.len() < content_length {
        let remaining = content_length - body.len();
        let mut chunk = vec![0u8; remaining.min(8192)];
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err("connection closed before the complete request body was received".into());
        }
        body.extend_from_slice(&chunk[..n]);
    }

    Ok((method, path, body))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn send_text_error(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    message: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    send_response(
        stream,
        code,
        reason,
        "text/plain; charset=utf-8",
        message.as_bytes(),
    )
}

fn send_response(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let header = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m3u8_path_extracts_full_id() {
        let path = "/m3u8/abc123";
        assert_eq!(&path[6..], "abc123");
    }

    #[test]
    fn need_server_is_case_insensitive_for_key() {
        assert!(needs_server("ush://play?needServer=1"));
        assert!(needs_server("ush://play?NEEDSERVER=1"));
        assert!(!needs_server("ush://play?needServer=0"));
        assert!(!needs_server("ush://play"));
    }
}
