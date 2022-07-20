#![allow(non_snake_case)]
#![allow(unused)]
#[macro_use]
extern crate serde_json;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

mod hive;
mod beerlover;
mod mongo;

use std::collections::HashSet;
use std::fmt::Debug;
use clap::Parser;

use crate::beerlover::Beerlover;
use crate::hive::{Counter, Hive, HiveEngine, HivePost};
use crate::mongo::{DatabaseOptions, StakingQueueEntry, Database as BeerDatabase};

/// Beerlover - Reward !BEER comments on the HIVE blockchain
#[derive(Parser, Debug)]
#[clap(author = "Developed by: wehmoen", version, about, long_about = None)]
struct CLIARGS {
    /// MongoDB connection URL
    #[clap(short = 'a', long, value_parser, default_value = "mongodb://127.0.0.1:27017")]
    mongodb_uri: String,
    /// MongoDB database name
    #[clap(short = 'b', long, value_parser, default_value = "beerlover")]
    mongodb_name: String,
    /// MongoDB collection name
    #[clap(short = 'c', long, value_parser, default_value = "beertransfers")]
    mongodb_collection: String,
    /// Broadcast API Host
    #[clap(short = 'd', long, value_parser, default_value = "http://127.0.0.1:6666/broacast")]
    broadcast_api_host: String,
    /// Hive RPC API Host
    #[clap(short = 'e', long, value_parser, default_value = "https://api.deathwing.me")]
    rpc_host: String,
    /// Hive Engine RPC API Host
    #[clap(short = 'f', long, value_parser, default_value = "https://ha.herpc.dtools.dev/contracts")]
    he_rpc_host: String,
    /// Hive Engine Token Symbol
    #[clap(short = 'g', long, value_parser, default_value = "BEER")]
    he_token_symbol: String,
    /// Hive Account
    #[clap(short = 'i', long, value_parser, default_value = "beerlover")]
    hive_account: String,
    /// Trigger word - Use this in a HIVE comment to share token
    #[clap(short = 'j', long, value_parser, default_value = "!BEER")]
    trigger_word: String,
    /// List of accounts to load the ignored user from and use them as blacklist. Comma seperated
    #[clap(short = 'k', long, value_parser, default_value = "beerlover,detlev,louis88,wehmoen")]
    banned_accounts: String,
    /// Share Ratio. Allow 1 Share per n token in wallet
    #[clap(short = 'l', long, value_parser, default_value_t = 24.0)]
    share_ration: f64,
    /// Reward amount. Number of token to be staked to parent author
    #[clap(short = 'm',long, value_parser, default_value = "0.100")]
    reward_amount: String,
}


#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args: CLIARGS = CLIARGS::parse();

    let mut counter = Counter::new(0);
    let mut he_counter = Counter::new(0);
    let client = reqwest::Client::new();

    let mut hive: Hive = Hive::new(args.rpc_host.clone(), client.clone(), counter);
    let mut hive_engine: HiveEngine = HiveEngine::new(args.he_rpc_host.clone(), client.clone(), he_counter);

    let db_options: DatabaseOptions = DatabaseOptions {
        uri: args.mongodb_uri.clone(),
        db_name: args.mongodb_name.clone(),
        collection_name: args.mongodb_collection.clone(),
    };

    let database: BeerDatabase = BeerDatabase::new(db_options).await;

    let banned_accounts: Vec<String> = args.banned_accounts.as_str().split(",").map(str::to_string).collect();
    let mut banned_account_names: Vec<String> = vec![];

    for account in banned_accounts {
        let account_names: Vec<String> = hive.get_ignore_list(account).await;
        banned_account_names = [&banned_account_names[..], &account_names[..]].concat()
    }

    let banned_words: Vec<&str> = vec![ //TODO: make configurable through cli arg like banned accounts
        "!PIZZA",
        "!LUV",
        "!ENGAGE",
    ];

    let beerlover: Beerlover = Beerlover::new(banned_account_names.clone(), banned_words.clone(), args.trigger_word.clone(), args.share_ration.clone());

    let start = beerlover.get_start_block();
    let hive_height = hive.get_head_block().await;

    let mut block_counter = Counter::new(start);

    debug!("Hive Account: \t\t{}", args.hive_account.clone());
    debug!("Hive RPC Host: \t\t{}", args.rpc_host.clone());
    debug!("Hive Engine RPC Host: \t{}", args.he_rpc_host.clone());
    debug!("Hive Engine Token Symbol: \t{}", args.he_token_symbol.clone());
    debug!("Hive Banned Accounts: \t{}", banned_account_names.clone().join(",").to_string());

    debug!("MongoDB URI: \t\t{}", &args.mongodb_uri);
    debug!("MongoDB Database: \t\t{}", &args.mongodb_name);
    debug!("MongoDB Collection: \t\t{}", &args.mongodb_collection);

    debug!("Beerlover Reward Amount: \t{} {}", &args.reward_amount, &args.he_token_symbol);
    debug!("Beerlover Share Ratio: \t{}", &args.share_ration);
    debug!("Beerlover Trigger Word: \t{}", &args.trigger_word);

    debug!("Beerlover Start Block: \t{}", &start);
    debug!("Beerlover Hive Head Block: \t{}", &hive_height);

    debug!("=============== BEERLOVER BEGIN ===============");

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

        info!("Block {} has {:?} transactions!", cur_block, &trx.len());

        for tx in trx {
            let posts: Vec<HivePost> = beerlover.filter_operations(tx["operations"].to_owned(), tx["transaction_id"].as_str().unwrap().to_string().to_owned());

            for post in posts {
                if database.already_processed(post.tx_id.clone()).await == false {
                    let author_beer_balance = hive_engine.stake(post.author.clone(), args.he_token_symbol.clone()).await;
                    let author_max_shares = beerlover.maxium_shares(author_beer_balance.clone());
                    let share_count = database.transfer_count(post.author.clone()).await;
                    let pending_share_count = database.pending_transfer_count(post.author.clone()).await;

                    let absolute_shares: i64 = share_count + pending_share_count;

                    if author_max_shares > 0 {
                        if absolute_shares < author_max_shares {
                            let entry: StakingQueueEntry = StakingQueueEntry {
                                from: post.author,
                                to: post.parent_author,
                                amount: args.reward_amount.clone(),
                                symbol: args.he_token_symbol.clone(),
                                permlink: post.parent_permlink.clone(),
                                from_tx: post.tx_id.clone(),
                            };

                            info!("New Queue Entry: {}", entry);

                            database.add_to_queue(entry).await;
                        }
                    }
                }
            }
        }

        beerlover.set_start_block(cur_block);

        if cur_block > hive_height {
            info!("Finished importing to headblock!");
            break;
        }
    }
}
