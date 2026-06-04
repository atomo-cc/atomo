//! Per-client outbound delivery with backpressure shedding.
//!
//! Each connected client has a bounded outbound queue. When a subscriber is too
//! slow to drain it, the ephemeral tier prefers to **shed frames** rather than
//! let one slow client stall the hub (and therefore every other subscriber).
//! Presence/live-fan-out traffic is high-frequency and disposable: a dropped
//! frame is superseded by the next one, so shedding is the right trade.

use tokio::sync::mpsc;

use crate::protocol::ServerMsg;

/// Default depth of a client's outbound queue before it starts shedding.
pub const DEFAULT_OUTBOUND_CAPACITY: usize = 256;

/// Outcome of attempting to enqueue one frame to a client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// Frame was queued for the client.
    Sent,
    /// Client's queue was full; the frame was shed (subscriber too slow).
    Dropped,
    /// Client is gone (receiver closed); caller should reap the connection.
    Closed,
}

/// Try to enqueue one frame without ever blocking the hub.
///
/// Never awaits: on a full queue the frame is dropped (the newest frame that
/// cannot fit is discarded; the slow subscriber simply misses it and catches up
/// on the following frames once it drains).
pub fn try_deliver(out: &mpsc::Sender<ServerMsg>, msg: ServerMsg) -> Delivery {
    match out.try_send(msg) {
        Ok(()) => Delivery::Sent,
        Err(mpsc::error::TrySendError::Full(_)) => Delivery::Dropped,
        Err(mpsc::error::TrySendError::Closed(_)) => Delivery::Closed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(message: &str) -> ServerMsg {
        ServerMsg::Error {
            message: message.into(),
        }
    }

    #[test]
    fn delivers_until_queue_is_full_then_sheds() {
        let (tx, _rx) = mpsc::channel(2);
        assert_eq!(try_deliver(&tx, frame("1")), Delivery::Sent);
        assert_eq!(try_deliver(&tx, frame("2")), Delivery::Sent);
        // Third frame has nowhere to go and the receiver is too slow to drain:
        // it is shed rather than blocking the hub.
        assert_eq!(try_deliver(&tx, frame("3")), Delivery::Dropped);
    }

    #[test]
    fn draining_frees_capacity_again() {
        let (tx, mut rx) = mpsc::channel(1);
        assert_eq!(try_deliver(&tx, frame("a")), Delivery::Sent);
        assert_eq!(try_deliver(&tx, frame("b")), Delivery::Dropped);
        let _ = rx.try_recv().expect("one buffered frame");
        assert_eq!(try_deliver(&tx, frame("c")), Delivery::Sent);
    }

    #[test]
    fn reports_closed_when_receiver_is_gone() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx);
        assert_eq!(try_deliver(&tx, frame("x")), Delivery::Closed);
    }
}
