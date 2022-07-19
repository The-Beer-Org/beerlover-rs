#[macro_use]
extern crate serde_json;

mod hive;
mod beerlover;
mod mongo;

use std::borrow::Borrow;
use std::fmt::Debug;
use std::io::ErrorKind::WouldBlock;
use clap::Parser;
use mongodb::sync::Database;
use crate::beerlover::Beerlover;
use crate::hive::{Counter, Hive, HiveEngine, HivePost};
use crate::mongo::{DatabaseOptions, StakingQueueEntry};
use crate::mongo::Database as BeerDatabase;

/// Beerlover - Reward !BEER comments on the HIVE blockchain
#[derive(Parser, Debug)]
#[clap(author = "wehmoen", version, about, long_about = None)]
struct CLIARGS {
    /// MongoDB connection URL
    #[clap(short = 'u', long, value_parser, default_value = "mongodb://127.0.0.1:27017")]
    mongodb_uri: String,
    /// MongoDB database name
    #[clap(short = 'd', long, value_parser, default_value = "beerlover")]
    mongodb_name: String,
    /// MongoDB collection name
    #[clap(short = 'c', long, value_parser, default_value = "beertransfers")]
    mongodb_collection: String,
    /// Broadcast API Host
    #[clap(short = 'b', long, value_parser, default_value = "http://127.0.0.1:6666/broacast")]
    broadcast_api_host: String,
    /// Hive RPC API Host
    #[clap(short = 'r', long, value_parser, default_value = "https://api.deathwing.me")]
    rpc_host: String,
    /// Hive Engine RPC API Host
    #[clap(short = 'e', long, value_parser, default_value = "https://ha.herpc.dtools.dev/contracts")]
    he_rpc_host: String,
    /// Hive Engine Token Symbol
    #[clap(short = 's', long, value_parser, default_value = "BEER")]
    he_token_symbol: String,
    /// Hive Account
    #[clap(short = 'a', long, value_parser, default_value = "beerlover")]
    hive_account: String,
    /// Trigger word - Use this in a HIVE comment to share token
    #[clap(short = 't', long, value_parser, default_value = "!BEER")]
    trigger_word: String,
    /// Banned accounts. Comma seperated
    #[clap(long, value_parser, default_value = "beerlover")]
    banned_accounts: String,
    /// Share Ratio. Allow 1 Share per n token in wallet
    #[clap(long, value_parser, default_value_t = 24.0)]
    share_ration: f64,
    /// Reward amount. Number of token to be staked to parent author
    #[clap(long, value_parser, default_value = "0.100")]
    reward_amount: String,
}


#[tokio::main]
async fn main() {
    let args: CLIARGS = CLIARGS::parse();

    println!("{:?}", args);

    let mut counter = Counter::new(0);
    let mut he_counter = Counter::new(0);
    let client = reqwest::Client::new();

    let mut hive: Hive = Hive::new(args.rpc_host, client.clone(), counter);
    let mut hive_engine: HiveEngine = HiveEngine::new(args.he_rpc_host, client.clone(), he_counter);

    let db_options: DatabaseOptions = DatabaseOptions {
        uri: args.mongodb_uri,
        db_name: args.mongodb_name,
        collection_name: args.mongodb_collection,
    };

    let database: BeerDatabase = BeerDatabase::new(db_options).await;

    let banned_accounts: Vec<String> = args.banned_accounts.as_str().split(",").map(str::to_string).collect();

    let banned_words: Vec<&str> = vec![ //TODO: make configurable through cli arg like banned accounts
                                        "!PIZZA",
                                        "!LUV",
                                        "!ENGAGE",
    ];

    let beerlover: Beerlover = Beerlover::new(banned_accounts, banned_words, args.trigger_word, args.share_ration);

    let start = beerlover.get_start_block();
    let hive_height = hive.get_head_block().await;

    let mut block_counter = Counter::new(start);

    loop {
        let cur_block = block_counter.next();
        let block_data = hive.get_block(cur_block).await;

        let trx = match block_data["result"]["transactions"].as_array() {
            Some(trx) => trx.to_owned(),
            _ => {
                beerlover.set_start_block(cur_block);
                continue;
            }
        };

        println!("Block {} has {:?} transactions!", cur_block, &trx.len());

        for tx in trx {
            let posts: Vec<HivePost> = beerlover.filter_operations(tx["operations"].to_owned(), tx["transaction_id"].as_str().unwrap().to_string().to_owned());

            for post in posts {
                if database.already_processed(post.tx_id.clone()).await == false {
                    let author_beer_balance = hive_engine.stake(post.author.clone(), args.he_token_symbol.clone()).await;
                    let author_max_shares = beerlover.maxium_shares(author_beer_balance.clone());
                    let share_count = database.transfer_count(post.author.clone()).await;

                    if author_max_shares > 0 {
                        if share_count < author_max_shares {

                            let entry: StakingQueueEntry = StakingQueueEntry {
                                from: post.author,
                                to: post.parent_author,
                                amount: args.reward_amount.clone(),
                                symbol: args.he_token_symbol.clone(),
                                permlink: post.parent_permlink.clone(),
                                from_tx: post.tx_id.clone()
                            };

                            println!("New Queue Entry: {:?}", entry);

                            database.add_to_queue(entry).ok();
                        }
                    }
                }
            }
        }

        beerlover.set_start_block(cur_block);

        if cur_block > hive_height {
            println!("Finished importing to headblock!");
            break;
        }
    }
}
