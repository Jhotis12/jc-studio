//! Unix domain socket listener.
//!
//! The socket is the trust boundary: `0600` inside a `0700` directory means only the
//! owning user can drive the daemon. A stale socket from a crash is detected by trying
//! to connect before removing it — never removed blindly, because a live daemon's socket
//! must never be stolen.

use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use hyper_util::rt::TokioIo;
use tokio::net::{UnixListener, UnixStream};
use tower::ServiceExt;

use crate::state::AppState;

pub async fn serve(
    state: AppState,
    socket: PathBuf,
    shutdown: impl Future<Output = ()>,
) -> Result<()> {
    prepare_socket(&socket).await?;

    let app = crate::http::router(state);
    let listener =
        UnixListener::bind(&socket).with_context(|| format!("cannot bind {}", socket.display()))?;
    restrict_permissions(&socket, 0o600)?;

    tracing::info!(socket = %socket.display(), "jcsd listening");

    let mut shutdown = std::pin::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                tracing::info!("shutdown requested");
                break;
            }
            accepted = listener.accept() => {
                let (stream, _) = match accepted {
                    Ok(pair) => pair,
                    Err(error) => {
                        tracing::warn!(error = %error, "failed to accept a connection");
                        continue;
                    }
                };
                let app = app.clone();
                tokio::spawn(async move {
                    if let Err(error) = serve_connection(stream, app).await {
                        tracing::debug!(error = %error, "connection closed");
                    }
                });
            }
        }
    }

    drop(listener);
    if let Err(error) = tokio::fs::remove_file(&socket).await {
        tracing::debug!(error = %error, "cannot remove the socket on shutdown");
    }
    Ok(())
}

async fn serve_connection(stream: UnixStream, app: axum::Router) -> Result<()> {
    let io = TokioIo::new(stream);
    let service = hyper::service::service_fn(move |request| {
        let app = app.clone();
        async move { Ok::<_, std::convert::Infallible>(app.oneshot(request).await.unwrap()) }
    });

    hyper::server::conn::http1::Builder::new()
        .serve_connection(io, service)
        .await
        .context("http connection failed")
}

/// Ensures the directory exists with tight permissions and no stale socket remains.
async fn prepare_socket(socket: &Path) -> Result<()> {
    if let Some(parent) = socket.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("cannot create {}", parent.display()))?;
            // Only tighten permissions on directories we created. Chmod-ing a
            // pre-existing directory could change something like `/tmp` itself.
            restrict_permissions(parent, 0o700)?;
        }
    }

    if socket.exists() {
        match UnixStream::connect(socket).await {
            Ok(_) => bail!(
                "another jcsd is already listening on {}; stop it first",
                socket.display()
            ),
            Err(_) => {
                tracing::warn!(
                    socket = %socket.display(),
                    "removing a stale socket left by a previous run"
                );
                tokio::fs::remove_file(socket).await?;
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
fn restrict_permissions(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(mode);
    std::fs::set_permissions(path, permissions)
        .with_context(|| format!("cannot set mode {mode:o} on {}", path.display()))
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}
