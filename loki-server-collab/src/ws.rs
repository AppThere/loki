// SPDX-License-Identifier: Apache-2.0

//! The WebSocket adapter: pumps frames between one client socket and a
//! [`DocRelay`]. Authentication and RBAC happen in the API layer *before*
//! the upgrade completes; this file only moves bytes.

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast::error::RecvError;

use crate::msg::CollabFrame;
use crate::relay::{DocRelay, RelayError};

/// Close code: the member's role forbids the attempted operation.
const CLOSE_POLICY: u16 = 1008;
/// Close code: server-side failure while relaying.
const CLOSE_INTERNAL: u16 = 1011;
/// Close code: the client fell too far behind and must resync from the
/// snapshot + oplog (its receiver lagged past the channel buffer).
const CLOSE_RESYNC: u16 = 1012;

async fn close(sink: &mut (impl SinkExt<Message> + Unpin), code: u16, reason: &'static str) {
    // Best-effort: the peer may already be gone.
    let _ = sink
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}

/// Runs one connection until the client disconnects or an error closes it.
///
/// `resume_after` is the last oplog sequence the client already holds
/// (`0` after a fresh snapshot download).
pub async fn drive_socket(socket: WebSocket, relay: DocRelay, resume_after: i64) {
    let (mut sink, mut stream) = socket.split();

    // Catch-up replay before live events (ADR-C013 recovery path).
    let mut rx = relay.subscribe().await;
    match relay.backlog(resume_after).await {
        Ok(frames) => {
            for frame in frames {
                if sink.send(Message::Binary(frame.encode().into())).await.is_err() {
                    return;
                }
            }
        }
        Err(error) => {
            tracing::warn!(%error, "backlog replay failed");
            close(&mut sink, CLOSE_INTERNAL, "backlog-failed").await;
            return;
        }
    }

    loop {
        tokio::select! {
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Binary(bytes))) => {
                    let frame = match CollabFrame::decode(&bytes) {
                        Ok(frame) => frame,
                        Err(error) => {
                            tracing::debug!(%error, "closing on malformed frame");
                            close(&mut sink, CLOSE_POLICY, "malformed-frame").await;
                            return;
                        }
                    };
                    match relay.ingest(frame).await {
                        Ok(()) => {}
                        Err(RelayError::WriteDenied) => {
                            close(&mut sink, CLOSE_POLICY, "write-denied").await;
                            return;
                        }
                        Err(error) => {
                            tracing::warn!(%error, "relay ingest failed");
                            close(&mut sink, CLOSE_INTERNAL, "ingest-failed").await;
                            return;
                        }
                    }
                }
                // Pings are answered by the WebSocket layer; text frames are
                // not part of the protocol and are ignored.
                Some(Ok(Message::Close(_))) | None => return,
                Some(Ok(_)) => {}
                Some(Err(error)) => {
                    tracing::debug!(%error, "socket read error");
                    return;
                }
            },
            event = rx.recv() => match event {
                Ok(event) if relay.wants(&event) => {
                    if sink
                        .send(Message::Binary(event.frame.encode().into()))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                Ok(_) => {} // This connection's own echo.
                Err(RecvError::Lagged(skipped)) => {
                    tracing::debug!(skipped, "subscriber lagged; forcing resync");
                    close(&mut sink, CLOSE_RESYNC, "resync-required").await;
                    return;
                }
                Err(RecvError::Closed) => return,
            },
        }
    }
}
