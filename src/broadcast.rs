use std::{collections::HashMap, sync::Arc, time::Duration};

use actix_web_lab::sse::{self, ChannelStream, Sse};
use parking_lot::Mutex;

use tokio::time::interval;

#[derive(Debug, Clone, Default)]
struct BroadcasterInner {
    clients: HashMap<String, Vec<sse::Sender>>,
}

pub struct BroadcastStore {
    inner: Mutex<BroadcasterInner>,
}

impl BroadcastStore {
    pub fn new() -> Arc<Self> {
        let this = Arc::new(BroadcastStore {
            inner: Mutex::new(BroadcasterInner::default()),
        });
        BroadcastStore::spawn_ping(Arc::clone(&this));
        this
    }

    fn spawn_ping(this: Arc<Self>) {
        actix_web::rt::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));

            loop {
                interval.tick().await;
                this.remove_stale_clients().await;
            }
        });
    }

    async fn remove_stale_clients(&self) {
        let clients = self.inner.lock().clients.clone();

        let mut ok_clients = HashMap::new();

        // iterate over all vectors in the hashmap and remove the stale clients
        for (channel, senders) in clients {
            let mut ok_senders = Vec::new();
            for sender in senders {
                if sender
                    .send(sse::Event::Comment("ping".into()))
                    .await
                    .is_ok()
                {
                    ok_senders.push(sender.clone());
                }
            }
            ok_clients.insert(channel, ok_senders);
        }

        self.inner.lock().clients = ok_clients;
    }

    pub async fn add_client(&self, id: String) -> Sse<ChannelStream> {
        let (tx, rx) = sse::channel(10);
        tx.send(sse::Data::new("connected")).await.unwrap();

        // add the client to the list of clients
        let mut inner = self.inner.lock();
        let x = inner.clients.entry(id.clone()).or_insert_with(Vec::new);
        x.push(tx);

        log::info!("added client to channel {}", id);

        rx
    }

    pub async fn broadcast(&self, channel: &str, msg: &str) {
        let inner = self.inner.lock();
        let recv = inner.clients.get(channel);

        match recv {
            Some(_) => {
                log::info!("broadcasting message to channel {}", channel);
                for sender in inner.clients.get(channel).unwrap() {
                    sender.send(sse::Data::new(msg)).await.unwrap();
                }
            }
            None => log::info!("channel {} not found", channel),
        }
    }

    pub fn client_count(&self, channel: &str) -> usize {
        let inner = self.inner.lock();
        let recv = inner.clients.get(channel);

        match recv {
            Some(recv) => recv.len(),
            None => 0,
        }
    }
}
