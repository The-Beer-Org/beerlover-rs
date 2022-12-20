use std::fmt;
use std::fmt::{Formatter};
use mongodb::{bson::doc, bson::DateTime, Collection, Client};
use serde::{Deserialize, Serialize};
use crate::{CLIARGS, HivePost};

const ONE_DAY: i64 = 60 * 60 * 24;

#[allow(dead_code)]
struct BeerTransfer {
    from: String,
    to: String,
    permlinkFrom: String,
    txIdFrom: String,
    txIdTo: String,
    createdAt: DateTime
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="lowercase")]
pub enum StakingQueueAction {
    StakeAndComment, //Everything okay. stake token and make comment
    NotEnoughTokenInAccount, // The main accounts does not have enougth token and needs a refill
    NotEnoughStake, // The user has not enough token staked
    SharesExceeded, // The user exceeded their 24 hour limit
    Blocked, // The user is blocked from using the service
    BlockedWord, // Post contains blacklisted word
    SelfReward, // User tries to give Beer to themselves
    Invalid // Used when the trigger word wasn't found. Not stored to db.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingQueueEntry {
    pub to: String,
    pub amount: String,
    pub symbol: String,
    pub from: String,
    pub permlink: String,
    pub from_permlink: String,
    pub from_tx: String,
    pub action: StakingQueueAction
}

impl fmt::Display for StakingQueueEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Stake {} {}\tFrom: {}\tTo: {}\tPermlink: {}\tFrom TX: {}", self.amount, self.symbol, self.from, self.to, self.permlink, self.from_tx)
    }
}

impl StakingQueueEntry {
    pub fn from(post: HivePost, args: &CLIARGS, action: StakingQueueAction) -> StakingQueueEntry {
        StakingQueueEntry {
            from: post.author,
            to: post.parent_author,
            amount: args.reward_amount.clone(),
            symbol: args.he_token_symbol.clone(),
            permlink: post.parent_permlink,
            from_permlink: post.permlink,
            from_tx: post.tx_id,
            action
        }
    }
}

pub struct DatabaseOptions {
    pub uri: String,
    pub db_name: String,
    pub collection_name: String,
    pub queue_collection_name: String
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
        let queue = database.collection::<StakingQueueEntry>(&options.queue_collection_name);

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

    pub async fn pending_transfer_count(&self, account: String) -> i64 {
        self.queue.count_documents(doc! {
            "from": account,
            "action": "stakeandcomment"
        }, None).await.unwrap_or(0) as i64
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
