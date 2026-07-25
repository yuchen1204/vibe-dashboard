use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::message::ServerMsg;

pub type ConnId = Uuid;

pub struct Hub {
    senders: DashMap<ConnId, mpsc::UnboundedSender<ServerMsg>>,
}

impl Hub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            senders: DashMap::new(),
        })
    }

    pub fn register(&self) -> (ConnId, mpsc::UnboundedReceiver<ServerMsg>) {
        let id = Uuid::new_v4();
        let (tx, rx) = mpsc::unbounded_channel();
        self.senders.insert(id, tx);
        tracing::info!(conn_id = %id, "ws connection registered");
        (id, rx)
    }

    pub fn unregister(&self, id: ConnId) {
        if self.senders.remove(&id).is_some() {
            tracing::info!(conn_id = %id, "ws connection unregistered");
        }
    }

    pub fn send_to(&self, id: ConnId, msg: ServerMsg) -> bool {
        if let Some(tx) = self.senders.get(&id) {
            tx.send(msg).is_ok()
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn broadcast(&self, msg: ServerMsg) {
        for entry in self.senders.iter() {
            let _ = entry.value().send(msg.clone());
        }
    }

    #[allow(dead_code)]
    pub fn connection_count(&self) -> usize {
        self.senders.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_returns_unique_ids() {
        let hub = Hub::new();
        let (id1, _rx1) = hub.register();
        let (id2, _rx2) = hub.register();
        assert_ne!(id1, id2);
        assert_eq!(hub.connection_count(), 2);
    }

    #[tokio::test]
    async fn unregister_removes_connection() {
        let hub = Hub::new();
        let (id, _rx) = hub.register();
        assert_eq!(hub.connection_count(), 1);
        hub.unregister(id);
        assert_eq!(hub.connection_count(), 0);
    }

    #[tokio::test]
    async fn send_to_delivers_message() {
        let hub = Hub::new();
        let (id, mut rx) = hub.register();
        let sent = hub.send_to(id, ServerMsg::pong());
        assert!(sent);
        let msg = rx.recv().await.expect("should receive");
        assert!(matches!(msg, ServerMsg::Pong { .. }));
    }

    #[tokio::test]
    async fn send_to_returns_false_for_unknown() {
        let hub = Hub::new();
        let sent = hub.send_to(Uuid::new_v4(), ServerMsg::pong());
        assert!(!sent);
    }

    #[tokio::test]
    async fn broadcast_reaches_all() {
        let hub = Hub::new();
        let (_id1, mut rx1) = hub.register();
        let (_id2, mut rx2) = hub.register();
        hub.broadcast(ServerMsg::pong());
        assert!(rx1.recv().await.is_some());
        assert!(rx2.recv().await.is_some());
    }
}