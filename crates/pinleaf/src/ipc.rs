use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::task::Waker;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcRequest {
    OpenPdf {
        path: PathBuf,
        page_one_based: Option<u32>,
    },
    SynctexForward {
        pdf_path: PathBuf,
        tex_path: PathBuf,
        line: u32,
        column: u32,
    },
    ExecuteCommand {
        command: String,
        args: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IpcResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Default)]
struct IpcInboxState {
    requests: Vec<IpcRequest>,
    waker: Option<Waker>,
}

#[derive(Clone, Default)]
pub struct IpcInbox(Arc<Mutex<IpcInboxState>>);

impl IpcInbox {
    pub fn push(&self, req: IpcRequest) {
        let mut guard = self.0.lock();
        guard.requests.push(req);
        if let Some(waker) = guard.waker.take() {
            waker.wake();
        }
    }

    pub async fn next_batch(&self) -> Vec<IpcRequest> {
        std::future::poll_fn(|cx| {
            let mut guard = self.0.lock();
            if guard.requests.is_empty() {
                guard.waker = Some(cx.waker().clone());
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(std::mem::take(&mut guard.requests))
            }
        })
        .await
    }
}

pub fn default_ipc_socket_path() -> PathBuf {
    let raw_user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "user".to_string());
    let sanitized: String = raw_user
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    std::env::temp_dir().join(format!("pinleaf-{sanitized}.sock"))
}

#[cfg(unix)]
pub fn send_ipc_request(sock_path: &Path, req: &IpcRequest) -> Result<IpcResponse> {
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let mut stream = UnixStream::connect(sock_path)
        .with_context(|| format!("failed to connect to Pinleaf IPC socket {}", sock_path.display()))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

    let mut line = serde_json::to_string(req)?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    let read_bytes = reader.read_line(&mut resp_line)?;
    if read_bytes == 0 {
        anyhow::bail!("empty response from Pinleaf IPC server");
    }
    let resp: IpcResponse = serde_json::from_str(resp_line.trim())?;
    Ok(resp)
}

#[cfg(not(unix))]
pub fn send_ipc_request(_sock_path: &Path, _req: &IpcRequest) -> Result<IpcResponse> {
    anyhow::bail!("IPC sockets are only supported on Unix platforms")
}

#[cfg(unix)]
pub fn start_ipc_listener(sock_path: PathBuf, inbox: IpcInbox) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    if sock_path.exists() {
        let ping = IpcRequest::ExecuteCommand {
            command: "ping".to_string(),
            args: Vec::new(),
        };
        if send_ipc_request(&sock_path, &ping).is_ok() {
            // Another active Pinleaf instance already owns this socket.
            return Ok(());
        }
        let _ = std::fs::remove_file(&sock_path);
    }

    let listener = UnixListener::bind(&sock_path)
        .with_context(|| format!("failed to bind IPC socket {}", sock_path.display()))?;
    let _ = std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600));

    std::thread::Builder::new()
        .name("pinleaf-ipc".to_string())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                    continue;
                }
                let mut writer = &stream;
                match serde_json::from_str::<IpcRequest>(line.trim()) {
                    Ok(IpcRequest::ExecuteCommand { command, args }) if command == "ping" => {
                        let _ = args;
                        let resp = IpcResponse {
                            ok: true,
                            message: "pong".to_string(),
                        };
                        if let Ok(mut s) = serde_json::to_string(&resp) {
                            s.push('\n');
                            let _ = writer.write_all(s.as_bytes());
                        }
                    }
                    Ok(req) => {
                        inbox.push(req);
                        let resp = IpcResponse {
                            ok: true,
                            message: "queued".to_string(),
                        };
                        if let Ok(mut s) = serde_json::to_string(&resp) {
                            s.push('\n');
                            let _ = writer.write_all(s.as_bytes());
                        }
                    }
                    Err(err) => {
                        let resp = IpcResponse {
                            ok: false,
                            message: format!("invalid IPC payload: {err}"),
                        };
                        if let Ok(mut s) = serde_json::to_string(&resp) {
                            s.push('\n');
                            let _ = writer.write_all(s.as_bytes());
                        }
                    }
                }
            }
        })?;

    Ok(())
}

#[cfg(not(unix))]
pub fn start_ipc_listener(_sock_path: PathBuf, _inbox: IpcInbox) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_client_server_round_trip() {
        let sock = std::env::temp_dir().join(format!("pinleaf-ipc-test-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&sock);
        let inbox = IpcInbox::default();
        start_ipc_listener(sock.clone(), inbox.clone()).unwrap();

        let ping_resp = send_ipc_request(
            &sock,
            &IpcRequest::ExecuteCommand {
                command: "ping".to_string(),
                args: vec![],
            },
        )
        .unwrap();
        assert!(ping_resp.ok);
        assert_eq!(ping_resp.message, "pong");

        let req = IpcRequest::ExecuteCommand {
            command: "goto_page".to_string(),
            args: vec!["4".to_string()],
        };
        let resp = send_ipc_request(&sock, &req).unwrap();
        assert!(resp.ok);
        assert_eq!(resp.message, "queued");

        let queued = std::mem::take(&mut inbox.0.lock().requests);
        assert_eq!(queued, vec![req]);

        let _ = std::fs::remove_file(&sock);
    }
}
