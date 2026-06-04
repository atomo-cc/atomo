//! Per-channel membership tracking with join/leave deltas.
//!
//! This is the single source of truth for "who is in which channel". The hub
//! delegates all membership questions here; it owns no membership maps of its
//! own. Everything is in-memory and per-node (the RFC's Phase-2 scope —
//! multi-node shared presence is a later phase).

use std::collections::{HashMap, HashSet};

use crate::hub::ClientId;
use crate::protocol::ChannelName;

/// One participant in a channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub client_id: ClientId,
    pub principal_id: String,
}

/// Channel membership for the whole hub.
#[derive(Debug, Default)]
pub struct Presence {
    /// channel -> (client_id -> principal_id)
    channels: HashMap<ChannelName, HashMap<ClientId, String>>,
}

impl Presence {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `client_id` (a given principal) joined `channel`.
    /// Returns `true` if this was a new membership (i.e. emit a `Joined`).
    pub fn join(
        &mut self,
        channel: &str,
        client_id: ClientId,
        principal_id: &str,
    ) -> bool {
        self.channels
            .entry(channel.to_string())
            .or_default()
            .insert(client_id, principal_id.to_string())
            .is_none()
    }

    /// Record that `client_id` left `channel`.
    /// Returns `true` if it had been a member (i.e. emit a `Left`).
    pub fn leave(&mut self, channel: &str, client_id: ClientId) -> bool {
        let Some(members) = self.channels.get_mut(channel) else {
            return false;
        };
        let removed = members.remove(&client_id).is_some();
        if members.is_empty() {
            self.channels.remove(channel);
        }
        removed
    }

    /// Current subscribers of a channel (client ids), for fan-out.
    pub fn subscribers(&self, channel: &str) -> Vec<ClientId> {
        self.channels
            .get(channel)
            .map(|m| m.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Membership snapshot for a channel — principal ids, sorted & de-duplicated
    /// (a principal may hold more than one connection).
    pub fn snapshot(&self, channel: &str) -> Vec<String> {
        let Some(members) = self.channels.get(channel) else {
            return Vec::new();
        };
        let set: HashSet<&String> = members.values().collect();
        let mut out: Vec<String> = set.into_iter().cloned().collect();
        out.sort();
        out
    }

    /// Number of channels with at least one member (gauge for metrics).
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}
