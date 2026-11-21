//! End-to-end socket test: spawn the real `jcsd` binary and drive it over a unix socket.
//!
//! No model and no OpenCode service are involved, so this runs anywhere. It covers the
//! parts that unit tests cannot: process startup, socket permissions, the HTTP framing
//! over UDS, event history, and single-owner exclusivity.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_util::rt::TokioIo;

struct Daemon {
    child: Child,
    socket: PathBuf,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_daemon(directory: &Path) -> Daemon {
    let socket = directory.join("jcsd.sock");
    let child = Command::new(env!("CARGO_BIN_EXE_jcsd"))
        .env("JCS_SOCKET", &socket)
        .env("JCS_DB", directory.join("jcs.db"))
        .env("XDG_DATA_HOME", directory.join("data"))
        .env("RUST_LOG", "error")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the daemon binary must be built for integration tests");

    Daemon { child, socket }
}

async fn wait_for_socket(socket: &Path) {
    for _ in 0..100 {
        if socket.exists() {
            // The file may exist before the listener accepts; retry the connection.
            if tokio::net::UnixStream::connect(socket).await.is_ok() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("daemon socket {} never became usable", socket.display());
}

async fn get_json(socket: &Path, path: &str) -> (u16, serde_json::Value) {
    let stream = tokio::net::UnixStream::connect(socket)
        .await
        .expect("connect to the daemon");
    let io = TokioIo::new(stream);

    let (mut sender, connection) = hyper::client::conn::http1::handshake(io)
        .await
        .expect("http handshake");
    tokio::spawn(async move {
        let _ = connection.await;
    });

    let request = hyper::Request::builder()
        .uri(path)
        .body(Full::new(Bytes::new()))
        .unwrap();

    let response = sender.send_request(request).await.expect("request");
    let status = response.status().as_u16();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
async fn the_daemon_serves_health_and_history_over_a_socket() {
    let directory = tempfile::tempdir().unwrap();
    let daemon = spawn_daemon(directory.path());
    wait_for_socket(&daemon.socket).await;

    let (status, health) = get_json(&daemon.socket, "/health").await;
    assert_eq!(status, 200);
    assert_eq!(health["ok"], true);
    assert_eq!(health["protocol"], jcs_proto::PROTOCOL_VERSION);
    assert!(health["database"].as_str().unwrap().ends_with("jcs.db"));

    // Startup writes an audit event before serving, so history is never empty.
    let (status, events) = get_json(&daemon.socket, "/events/history?limit=10").await;
    assert_eq!(status, 200);
    let kinds: Vec<&str> = events
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|event| event["kind"].as_str())
        .collect();
    assert!(
        kinds.contains(&"daemon_started"),
        "expected a daemon_started event, got {kinds:?}"
    );

    // The socket is private to the user.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&daemon.socket)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "socket mode was {mode:o}");
    }
}

#[tokio::test]
async fn a_second_daemon_refuses_to_steal_a_live_socket() {
    let directory = tempfile::tempdir().unwrap();
    let _first = spawn_daemon(directory.path());
    wait_for_socket(&_first.socket).await;

    // Same socket: the second process must exit non-zero and leave the first alive.
    let mut second = Command::new(env!("CARGO_BIN_EXE_jcsd"))
        .env("JCS_SOCKET", &_first.socket)
        .env("JCS_DB", directory.path().join("other.db"))
        .env("XDG_DATA_HOME", directory.path().join("data2"))
        .env("RUST_LOG", "error")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn the second daemon");

    let exit = tokio::task::spawn_blocking(move || second.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(
        !exit.success(),
        "a second daemon must not steal a live socket"
    );

    // The original is still serving.
    let (status, _) = get_json(&_first.socket, "/health").await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn a_stale_socket_from_a_killed_daemon_is_replaced() {
    let directory = tempfile::tempdir().unwrap();

    // First daemon starts and is killed without cleanup.
    let first = spawn_daemon(directory.path());
    wait_for_socket(&first.socket).await;
    let socket = first.socket.clone();
    drop(first);
    assert!(
        socket.exists(),
        "a hard kill should leave the socket behind"
    );

    // Second daemon must detect the stale socket, remove it and bind again.
    let second = spawn_daemon(directory.path());
    wait_for_socket(&second.socket).await;

    let (status, health) = get_json(&second.socket, "/health").await;
    assert_eq!(status, 200);
    assert_eq!(health["ok"], true);
}
