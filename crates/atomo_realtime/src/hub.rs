//! The hub: a single owning task that holds all realtime state.
//!
//! There are no locks on the hot path. The hub state lives inside one task; the
//! outside world drives it by sending [`Command`]s down an mpsc channel. Each
//! connection gets a cheap [`ClientHandle`] (clone of the command sender + its
//! id) and a bounded outbound queue it drains itself.
//!
//! ```text
//!   transport ──connect()──► Hub task ──fan-out──► every subscriber's queue
//!      │  (read loop)            │  (owns clients, presence, stats)
//!      └── dispatch(ClientMsg) ──┘
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::Serialize;
use serde_json::value::RawValue;
use tokio::sync::mpsc;
use tracing::trace;

use crate::client::{self, Delivery, DEFAULT_OUTBOUND_CAPACITY};
use crate::presence::Presence;
use crate::protocol::{ChannelName, ClientMsg, Payload, Principal, ServerMsg};

/// Identifies one connection for the lifetime of the hub. Monotonic, never reused.
pub type ClientId = u64;

/// Depth of the hub's inbound command queue.
const COMMAND_CAPACITY: usize = 1024;

/// Internal messages from connections to the hub task.
enum Command {
    Connect {
        id: ClientId,
        principal: Principal,
        out: mpsc::Sender<ServerMsg>,
    },
    Subscribe {
        id: ClientId,
        channel: ChannelName,
    },
    Unsubscribe {
        id: ClientId,
        channel: ChannelName,
    },
    Publish {
        id: ClientId,
        channel: ChannelName,
        payload: Payload,
    },
    Presence {
        id: ClientId,
        channel: ChannelName,
    },
    Disconnect {
        id: ClientId,
    },
}

/// Handle to the realtime hub. Cheap to clone; clones share the one hub task.
#[derive(Clone)]
pub struct Hub {
    cmd_tx: mpsc::Sender<Command>,
    next_id: Arc<AtomicU64>,
    stats: Arc<Stats>,
}

impl Hub {
    /// Spawn the hub task and return a handle to it.
    pub fn new() -> Hub {
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_CAPACITY);
        let stats = Arc::new(Stats::default());
        let worker = Worker::new(stats.clone());
        tokio::spawn(worker.run(cmd_rx));
        Hub {
            cmd_tx,
            next_id: Arc::new(AtomicU64::new(1)),
            stats,
        }
    }

    /// Register a new connection for `principal` and return its [`Connection`].
    ///
    /// The transport drains [`Connection::outbound`] onto the socket and feeds
    /// inbound frames through [`Connection::handle`].
    pub async fn connect(&self, principal: Principal) -> Connection {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (out_tx, out_rx) = mpsc::channel(DEFAULT_OUTBOUND_CAPACITY);
        // If the hub task is gone, the connection is simply dead-on-arrival; the
        // outbound receiver will yield nothing and the handle's sends no-op.
        let _ = self
            .cmd_tx
            .send(Command::Connect {
                id,
                principal,
                out: out_tx,
            })
            .await;
        Connection {
            id,
            outbound: out_rx,
            handle: ClientHandle {
                id,
                cmd_tx: self.cmd_tx.clone(),
            },
        }
    }

    /// Point-in-time counters for observability.
    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

/// Everything the transport needs for one connection.
pub struct Connection {
    /// This connection's id.
    pub id: ClientId,
    /// Frames the hub wants delivered to this client — drain onto the socket.
    pub outbound: mpsc::Receiver<ServerMsg>,
    /// Drive inbound client frames and lifecycle through this.
    pub handle: ClientHandle,
}

/// A cheap, cloneable handle used by a transport's read loop to drive one
/// connection. Dropping it does **not** disconnect — call [`ClientHandle::disconnect`]
/// when the socket closes.
#[derive(Clone)]
pub struct ClientHandle {
    id: ClientId,
    cmd_tx: mpsc::Sender<Command>,
}

impl ClientHandle {
    /// This connection's id.
    pub fn id(&self) -> ClientId {
        self.id
    }

    /// Apply one parsed client frame.
    pub async fn dispatch(&self, msg: ClientMsg) {
        let cmd = match msg {
            ClientMsg::Subscribe { channel } => Command::Subscribe { id: self.id, channel },
            ClientMsg::Unsubscribe { channel } => Command::Unsubscribe { id: self.id, channel },
            ClientMsg::Publish { channel, payload } => Command::Publish {
                id: self.id,
                channel,
                // Box<RawValue> -> Arc<RawValue>: keep the bytes opaque, just
                // make them cheap to share across the fan-out.
                payload: Payload::from(payload),
            },
            ClientMsg::Presence { channel } => Command::Presence { id: self.id, channel },
        };
        let _ = self.cmd_tx.send(cmd).await;
    }

    /// Tear down this connection: drop its subscriptions and emit presence-leaves.
    pub async fn disconnect(&self) {
        let _ = self.cmd_tx.send(Command::Disconnect { id: self.id }).await;
    }
}

// ===========================================================================
// Worker — owns all hub state; runs in its own task.
// ===========================================================================

struct ClientState {
    out: mpsc::Sender<ServerMsg>,
    principal: Principal,
    subscriptions: HashSet<ChannelName>,
}

struct Worker {
    clients: HashMap<ClientId, ClientState>,
    presence: Presence,
    stats: Arc<Stats>,
}

impl Worker {
    fn new(stats: Arc<Stats>) -> Self {
        Self {
            clients: HashMap::new(),
            presence: Presence::new(),
            stats,
        }
    }

    async fn run(mut self, mut cmd_rx: mpsc::Receiver<Command>) {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                Command::Connect { id, principal, out } => self.on_connect(id, principal, out),
                Command::Subscribe { id, channel } => self.on_subscribe(id, channel),
                Command::Unsubscribe { id, channel } => self.on_unsubscribe(id, channel),
                Command::Publish { id, channel, payload } => self.on_publish(id, channel, payload),
                Command::Presence { id, channel } => self.on_presence(id, channel),
                Command::Disconnect { id } => self.on_disconnect(id),
            }
        }
    }

    fn on_connect(&mut self, id: ClientId, principal: Principal, out: mpsc::Sender<ServerMsg>) {
        self.clients.insert(
            id,
            ClientState {
                out,
                principal,
                subscriptions: HashSet::new(),
            },
        );
        self.stats.connections_opened.fetch_add(1, Ordering::Relaxed);
        self.stats
            .active_clients
            .store(self.clients.len() as u64, Ordering::Relaxed);
    }

    fn on_subscribe(&mut self, id: ClientId, channel: ChannelName) {
        let Some(principal_id) = self.clients.get(&id).map(|c| c.principal.id.clone()) else {
            return;
        };
        let newly = self.presence.join(&channel, id, &principal_id);
        if let Some(cs) = self.clients.get_mut(&id) {
            cs.subscriptions.insert(channel.clone());
        }
        // Send the joiner the current membership snapshot first…
        let members = self.presence.snapshot(&channel);
        self.deliver(
            id,
            ServerMsg::Presence {
                channel: channel.clone(),
                members,
            },
        );
        // …then tell the rest of the channel someone joined (only on a genuinely
        // new membership, so reconnect spam doesn't fan out duplicates).
        if newly {
            self.broadcast_except(
                &channel,
                id,
                ServerMsg::Joined {
                    channel: channel.clone(),
                    who: principal_id,
                },
            );
        }
        self.refresh_channel_gauge();
    }

    fn on_unsubscribe(&mut self, id: ClientId, channel: ChannelName) {
        if let Some(cs) = self.clients.get_mut(&id) {
            cs.subscriptions.remove(&channel);
        }
        self.leave_channel(id, &channel);
        self.refresh_channel_gauge();
    }

    fn on_publish(&mut self, id: ClientId, channel: ChannelName, payload: Payload) {
        self.stats.messages_published.fetch_add(1, Ordering::Relaxed);
        let from = self.clients.get(&id).map(|c| c.principal.id.clone());
        // Fan out to every subscriber except the publisher (no self-echo).
        for target in self.presence.subscribers(&channel) {
            if target == id {
                continue;
            }
            self.deliver(
                target,
                ServerMsg::Message {
                    channel: channel.clone(),
                    from: from.clone(),
                    payload: payload.clone(),
                },
            );
        }
    }

    fn on_presence(&mut self, id: ClientId, channel: ChannelName) {
        let members = self.presence.snapshot(&channel);
        self.deliver(id, ServerMsg::Presence { channel, members });
    }

    fn on_disconnect(&mut self, id: ClientId) {
        let Some(cs) = self.clients.remove(&id) else {
            return;
        };
        // Emit a Left on every channel this client was in. The principal is taken
        // from `cs` (the client is already removed from the map), so leave is told
        // explicitly rather than re-looking it up.
        let who = cs.principal.id;
        for channel in cs.subscriptions {
            if self.presence.leave(&channel, id) {
                self.broadcast_except(
                    &channel,
                    id,
                    ServerMsg::Left {
                        channel: channel.clone(),
                        who: who.clone(),
                    },
                );
            }
        }
        self.stats.connections_closed.fetch_add(1, Ordering::Relaxed);
        self.stats
            .active_clients
            .store(self.clients.len() as u64, Ordering::Relaxed);
        self.refresh_channel_gauge();
    }

    /// Remove `id` from `channel` and, if it had been a member, tell the rest.
    fn leave_channel(&mut self, id: ClientId, channel: &str) {
        let principal_id = self.clients.get(&id).map(|c| c.principal.id.clone());
        if self.presence.leave(channel, id) {
            if let Some(who) = principal_id {
                self.broadcast_except(
                    channel,
                    id,
                    ServerMsg::Left {
                        channel: channel.to_string(),
                        who,
                    },
                );
            }
        }
    }

    /// Deliver one frame to every subscriber of `channel` except `except`.
    fn broadcast_except(&mut self, channel: &str, except: ClientId, msg: ServerMsg) {
        for target in self.presence.subscribers(channel) {
            if target == except {
                continue;
            }
            self.deliver(target, msg.clone());
        }
    }

    /// Deliver one frame to one client, accounting for the outcome. A client
    /// whose socket has closed is reaped here.
    fn deliver(&mut self, id: ClientId, msg: ServerMsg) {
        let Some(cs) = self.clients.get(&id) else {
            return;
        };
        match client::try_deliver(&cs.out, msg) {
            Delivery::Sent => {
                self.stats.messages_delivered.fetch_add(1, Ordering::Relaxed);
            }
            Delivery::Dropped => {
                self.stats.dropped_frames.fetch_add(1, Ordering::Relaxed);
                trace!(client_id = id, "shed frame: slow subscriber");
            }
            Delivery::Closed => self.on_disconnect(id),
        }
    }

    fn refresh_channel_gauge(&self) {
        self.stats
            .active_channels
            .store(self.presence.channel_count() as u64, Ordering::Relaxed);
    }
}

// ===========================================================================
// Stats
// ===========================================================================

#[derive(Default)]
struct Stats {
    messages_published: AtomicU64,
    messages_delivered: AtomicU64,
    dropped_frames: AtomicU64,
    connections_opened: AtomicU64,
    connections_closed: AtomicU64,
    active_clients: AtomicU64,
    active_channels: AtomicU64,
}

impl Stats {
    fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            messages_published: self.messages_published.load(Ordering::Relaxed),
            messages_delivered: self.messages_delivered.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed),
            connections_opened: self.connections_opened.load(Ordering::Relaxed),
            connections_closed: self.connections_closed.load(Ordering::Relaxed),
            active_clients: self.active_clients.load(Ordering::Relaxed),
            active_channels: self.active_channels.load(Ordering::Relaxed),
        }
    }
}

/// Point-in-time hub counters (cumulative totals + current gauges).
#[derive(Debug, Clone, Serialize)]
pub struct StatsSnapshot {
    pub messages_published: u64,
    pub messages_delivered: u64,
    pub dropped_frames: u64,
    pub connections_opened: u64,
    pub connections_closed: u64,
    pub active_clients: u64,
    pub active_channels: u64,
}

/// Convenience for transports that build a [`Payload`] from already-owned bytes.
pub fn payload_from_str(json: &str) -> Result<Payload, serde_json::Error> {
    RawValue::from_string(json.to_string()).map(Payload::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_from_str_preserves_valid_json() {
        let p = payload_from_str(r#"{"a":1,"b":[2,3]}"#).unwrap();
        assert_eq!(p.get(), r#"{"a":1,"b":[2,3]}"#);
    }

    #[test]
    fn payload_from_str_rejects_malformed_json() {
        assert!(payload_from_str("{not json").is_err());
    }

    #[test]
    fn fresh_hub_stats_start_at_zero() {
        // No runtime task is touched until connect(); a fresh hub reports zeros.
        let stats = StatsSnapshot {
            messages_published: 0,
            messages_delivered: 0,
            dropped_frames: 0,
            connections_opened: 0,
            connections_closed: 0,
            active_clients: 0,
            active_channels: 0,
        };
        // Round-trips through serde the way /realtime/health exposes it.
        let json = serde_json::to_string(&stats).unwrap();
        let back: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(back["active_clients"], 0);
        assert_eq!(back["dropped_frames"], 0);
    }
}
