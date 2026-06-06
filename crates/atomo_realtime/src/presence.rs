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
    pub fn join(&mut self, channel: &str, client_id: ClientId, principal_id: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_reports_only_the_first_membership_as_new() {
        let mut p = Presence::new();
        assert!(p.join("room", 1, "alice"), "first join is new");
        assert!(
            !p.join("room", 1, "alice"),
            "re-join of same client is not new"
        );
        assert!(p.join("room", 2, "bob"), "different client is new");
    }

    #[test]
    fn subscribers_lists_every_client_unknown_channel_is_empty() {
        let mut p = Presence::new();
        p.join("room", 1, "alice");
        p.join("room", 2, "bob");
        let mut subs = p.subscribers("room");
        subs.sort();
        assert_eq!(subs, vec![1, 2]);
        assert!(p.subscribers("nope").is_empty());
    }

    #[test]
    fn snapshot_dedups_principals_across_connections_and_sorts() {
        let mut p = Presence::new();
        // Same principal "alice" on two different connections; "bob" once.
        p.join("room", 1, "alice");
        p.join("room", 2, "alice");
        p.join("room", 3, "bob");
        assert_eq!(
            p.snapshot("room"),
            vec!["alice".to_string(), "bob".to_string()]
        );
        assert!(p.snapshot("missing").is_empty());
    }

    #[test]
    fn leave_reports_membership_and_prunes_empty_channels() {
        let mut p = Presence::new();
        p.join("room", 1, "alice");
        assert_eq!(p.channel_count(), 1);

        assert!(
            p.leave("room", 1),
            "leaving an existing member returns true"
        );
        assert!(!p.leave("room", 1), "leaving again returns false");
        assert!(
            !p.leave("ghost", 9),
            "leaving an unknown channel returns false"
        );
        assert_eq!(p.channel_count(), 0, "empty channel is pruned");
    }

    #[test]
    fn channel_only_pruned_once_last_member_leaves() {
        let mut p = Presence::new();
        p.join("room", 1, "alice");
        p.join("room", 2, "bob");
        assert!(p.leave("room", 1));
        assert_eq!(p.channel_count(), 1, "still has bob");
        assert_eq!(p.snapshot("room"), vec!["bob".to_string()]);
        assert!(p.leave("room", 2));
        assert_eq!(p.channel_count(), 0);
    }
}
