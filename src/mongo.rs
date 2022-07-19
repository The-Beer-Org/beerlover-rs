use chrono::NaiveDateTime;
use mongodb::{bson::doc, bson::DateTime, Collection, Client, options::FindOneOptions, IndexModel};
use mongodb::options::{IndexOptions, InsertManyOptions};
use serde::{Deserialize, Serialize};

const ONE_DAY: i64 = 60 * 60 * 24;

#[derive(Debug, Clone)]
struct BeerTransfer {
    from: String,
    to: String,
    permlinkFrom: String,
    txIdFrom: String,
    txIdTo: String,
    createdAt: DateTime
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingQueueEntry {
    pub to: String,
    pub amount: String,
    pub symbol: String,
    pub from: String,
    pub permlink: String,
    pub from_tx: String
}

pub struct DatabaseOptions {
    pub uri: String,
    pub db_name: String,
    pub collection_name: String
}

pub struct Database {
    client: Client,
    collection: Collection<BeerTransfer>,
    queue: Collection<StakingQueueEntry>
}

impl Database {
    pub async fn new(options: DatabaseOptions) -> Database {
        let client = Client::with_uri_str(&options.uri).await.unwrap();
        let database = client.database(&options.db_name);
        let collection = database.collection::<BeerTransfer>(&options.collection_name);
        let queue = database.collection::<StakingQueueEntry>("queue");

        Database {
            client,
            collection,
            queue
        }
    }

    pub async fn already_processed(&self, tx_id: String) -> bool {
        self.collection.count_documents(doc! {
            "txIdFrom": tx_id
        }, None).await.unwrap() > 0
    }

    pub async fn add_to_queue(&self, entry: StakingQueueEntry) {
        self.queue.insert_one(entry, None).await.ok();
    }

    pub async fn transfer_count(&self, account: String) -> i64 {

        let yesterday = DateTime::from_millis((chrono::Utc::now().timestamp() - ONE_DAY) * 1000);

        self.collection.count_documents(doc! {
            "createdAt": {
                "$gt": yesterday
            },
            "from": account
        }, None).await.unwrap_or(0) as i64

    }
}
