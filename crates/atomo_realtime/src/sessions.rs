//! Coordinator sessions — a room with one authoritative member (the
//! *coordinator*) through which the others relay.
//!
//! This is the domain-agnostic primitive behind host-authoritative workloads: a
//! relay backend where one participant runs the source of truth (a game host
//! running the simulation, a collab session with a single authority) and the
//! rest send to it / receive from it. The hub owns a [`Sessions`] and emits the
//! wire frames; this module is pure bookkeeping (no I/O, no domain types).

use std::collections::HashMap;

use crate::hub::ClientId;

/// What happens to a session when its coordinator leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoordinatorLeavePolicy {
    /// Promote the next-oldest member to coordinator (graceful host migration).
    #[default]
    Reelect,
    /// End the session and notify the remaining members.
    Close,
}

/// One participant in a session. `slot` is a stable per-session index assigned
/// at join and never reused or shifted, so apps can key player/seat state on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMember {
    pub client_id: ClientId,
    pub principal_id: String,
    pub slot: u32,
}

struct Session {
    /// Join order; `order[0]` is the next coordinator under `Reelect`.
    order: Vec<ClientId>,
    members: HashMap<ClientId, SessionMember>,
    coordinator: ClientId,
    next_slot: u32,
}

impl Session {
    /// Members sorted by slot (stable roster order).
    fn roster(&self) -> Vec<SessionMember> {
        let mut v: Vec<SessionMember> = self.members.values().cloned().collect();
        v.sort_by_key(|m| m.slot);
        v
    }
}

/// Result of joining a session.
#[derive(Debug, Clone)]
pub struct JoinOutcome {
    pub slot: u32,
    pub is_coordinator: bool,
    /// The full roster after the join (includes the joiner).
    pub roster: Vec<SessionMember>,
}

/// How a leave affected the coordinator role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatorChange {
    /// The member who left was not the coordinator.
    None,
    /// The coordinator left and a new one was elected (`Reelect`).
    Reelected(SessionMember),
    /// The coordinator left and the session was ended (`Close`).
    Closed,
}

/// Result of leaving (or being disconnected from) a session.
#[derive(Debug, Clone)]
pub struct LeaveOutcome {
    /// The member that left.
    pub member: SessionMember,
    pub change: CoordinatorChange,
    /// Members still in the session (empty when `removed`); the set to notify.
    pub remaining: Vec<SessionMember>,
    /// True when the session no longer exists (went empty, or was closed).
    pub removed: bool,
}

/// All coordinator sessions in the hub. Keyed by session name.
#[derive(Default)]
pub struct Sessions {
    map: HashMap<String, Session>,
}

impl Sessions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Join (creating the session if new — the first joiner is the coordinator).
    /// Idempotent: re-joining returns the existing slot/role.
    pub fn join(&mut self, name: &str, client_id: ClientId, principal_id: &str) -> JoinOutcome {
        let session = self.map.entry(name.to_string()).or_insert_with(|| Session {
            order: Vec::new(),
            members: HashMap::new(),
            coordinator: client_id,
            next_slot: 0,
        });
        if let Some(m) = session.members.get(&client_id) {
            return JoinOutcome {
                slot: m.slot,
                is_coordinator: session.coordinator == client_id,
                roster: session.roster(),
            };
        }
        let slot = session.next_slot;
        session.next_slot += 1;
        session.members.insert(
            client_id,
            SessionMember {
                client_id,
                principal_id: principal_id.to_string(),
                slot,
            },
        );
        session.order.push(client_id);
        JoinOutcome {
            slot,
            is_coordinator: session.coordinator == client_id,
            roster: session.roster(),
        }
    }

    /// Leave a session, applying `policy` if the coordinator is the one leaving.
    /// Returns `None` if the client was not a member.
    pub fn leave(
        &mut self,
        name: &str,
        client_id: ClientId,
        policy: CoordinatorLeavePolicy,
    ) -> Option<LeaveOutcome> {
        // Scope the &mut borrow so we can remove the session from the map after.
        enum Post {
            Keep,
            Remove,
        }
        let (member, change, remaining, post) = {
            let session = self.map.get_mut(name)?;
            let member = session.members.remove(&client_id)?;
            session.order.retain(|c| *c != client_id);
            let was_coordinator = session.coordinator == client_id;

            if session.members.is_empty() {
                (member, CoordinatorChange::None, Vec::new(), Post::Remove)
            } else if !was_coordinator {
                (
                    member,
                    CoordinatorChange::None,
                    session.roster(),
                    Post::Keep,
                )
            } else {
                match policy {
                    CoordinatorLeavePolicy::Reelect => {
                        let new_id = session.order[0];
                        session.coordinator = new_id;
                        let new_member = session.members[&new_id].clone();
                        (
                            member,
                            CoordinatorChange::Reelected(new_member),
                            session.roster(),
                            Post::Keep,
                        )
                    }
                    CoordinatorLeavePolicy::Close => (
                        member,
                        CoordinatorChange::Closed,
                        session.roster(),
                        Post::Remove,
                    ),
                }
            }
        };
        let removed = matches!(post, Post::Remove);
        if removed {
            self.map.remove(name);
        }
        Some(LeaveOutcome {
            member,
            change,
            remaining,
            removed,
        })
    }

    pub fn coordinator(&self, name: &str) -> Option<ClientId> {
        self.map.get(name).map(|s| s.coordinator)
    }

    pub fn is_coordinator(&self, name: &str, client_id: ClientId) -> bool {
        self.map
            .get(name)
            .is_some_and(|s| s.coordinator == client_id)
    }

    pub fn member(&self, name: &str, client_id: ClientId) -> Option<&SessionMember> {
        self.map.get(name).and_then(|s| s.members.get(&client_id))
    }

    pub fn roster(&self, name: &str) -> Vec<SessionMember> {
        self.map.get(name).map(Session::roster).unwrap_or_default()
    }

    /// Number of live sessions (gauge for metrics).
    pub fn count(&self) -> usize {
        self.map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_joiner_is_coordinator_others_get_stable_slots() {
        let mut s = Sessions::new();
        let a = s.join("m", 1, "alice");
        assert!(a.is_coordinator);
        assert_eq!(a.slot, 0);

        let b = s.join("m", 2, "bob");
        assert!(!b.is_coordinator);
        assert_eq!(b.slot, 1);
        assert_eq!(b.roster.len(), 2);
    }

    #[test]
    fn slots_are_stable_when_an_earlier_member_leaves() {
        let mut s = Sessions::new();
        s.join("m", 1, "alice");
        s.join("m", 2, "bob");
        let c = s.join("m", 3, "carol");
        assert_eq!(c.slot, 2);

        // Bob (slot 1) leaves; Carol keeps slot 2 (no shifting).
        s.leave("m", 2, CoordinatorLeavePolicy::Reelect);
        assert_eq!(s.member("m", 3).unwrap().slot, 2);
    }

    #[test]
    fn reelect_promotes_the_oldest_remaining_member() {
        let mut s = Sessions::new();
        s.join("m", 1, "alice"); // coordinator
        s.join("m", 2, "bob");
        s.join("m", 3, "carol");

        let out = s.leave("m", 1, CoordinatorLeavePolicy::Reelect).unwrap();
        match out.change {
            CoordinatorChange::Reelected(m) => assert_eq!(m.client_id, 2), // bob, oldest remaining
            other => panic!("expected reelection, got {other:?}"),
        }
        assert!(!out.removed);
        assert!(s.is_coordinator("m", 2));
    }

    #[test]
    fn close_policy_ends_the_session_when_coordinator_leaves() {
        let mut s = Sessions::new();
        s.join("m", 1, "alice"); // coordinator
        s.join("m", 2, "bob");

        let out = s.leave("m", 1, CoordinatorLeavePolicy::Close).unwrap();
        assert_eq!(out.change, CoordinatorChange::Closed);
        assert!(out.removed);
        assert_eq!(
            out.remaining
                .iter()
                .map(|m| m.client_id)
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(s.count(), 0);
    }

    #[test]
    fn last_member_leaving_removes_the_session() {
        let mut s = Sessions::new();
        s.join("m", 1, "alice");
        let out = s.leave("m", 1, CoordinatorLeavePolicy::Reelect).unwrap();
        assert!(out.removed);
        assert_eq!(s.count(), 0);
    }

    #[test]
    fn leaving_a_non_member_or_missing_session_is_none() {
        let mut s = Sessions::new();
        s.join("m", 1, "alice");
        assert!(s.leave("m", 99, CoordinatorLeavePolicy::Reelect).is_none());
        assert!(s
            .leave("ghost", 1, CoordinatorLeavePolicy::Reelect)
            .is_none());
    }
}
