use std::path::Path;
use std::fs;
use serde_json::Value;
use crate::{hive, StakingQueueAction};
use crate::hive::{HivePost, HivePostList};
pub struct Beerlover {
    banned_accounts: Vec<String>,
    banned_words: Vec<&'static str>,
    command: String,
    share_ratio: f64,
}

impl Beerlover {
    pub fn new(banned_accounts: Vec<String>, banned_words: Vec<&'static str>, command: String, share_ratio: f64) -> Beerlover {
        Beerlover {
            banned_accounts,
            banned_words,
            command,
            share_ratio,
        }
    }

    pub fn maxium_shares(&self, balance: f64) -> i64 {
        (balance / self.share_ratio) as i64
    }

    pub fn filter_operations(&self, operations: serde_json::Value, tx_id: String) -> HivePostList {
        let op_array: Option<&Vec<Value>> = operations.as_array().to_owned();

        let mut valid_posts: HivePostList = vec![];

        for op in op_array.iter() {
            let op_self: Vec<Value> = op.to_owned().to_owned();
            let op_name: &str = op_self[0][0].as_str().unwrap(); // Possibly do one more iteration for tx with more than one op tuple

            if op_name == hive::hive_ops::COMMENT  {
                let mut post: HivePost = HivePost::from(op_self[0].to_owned(), tx_id.to_owned(), StakingQueueAction::Invalid);

                if post.body.contains(self.command.clone().as_str()) {
                    let mut valid = true;

                    if self.banned_accounts.contains(&post.author) == false && self.banned_accounts.contains(&post.parent_author) == false {
                        for word in self.banned_words.to_owned() {
                            if post.body.contains(word) {
                                post.action = StakingQueueAction::BlockedWord;
                                valid_posts.push(post.clone());
                                valid = false;
                            }
                        }
                    } else {
                        post.action = StakingQueueAction::Blocked;
                        valid_posts.push(post.clone());
                        valid = false;
                    }

                    if post.author == post.parent_author || post.parent_permlink == "" || post.parent_author == "" {
                        post.action = StakingQueueAction::SelfReward;
                        valid_posts.push(post.clone());
                        valid = false;
                    }

                    if valid  {
                        post.action = StakingQueueAction::StakeAndComment;
                        valid_posts.push(post.clone());
                    }
                }
            }
        }

        valid_posts
    }

    pub fn get_start_block(&self) -> i64 {
        let re = regex::Regex::new(r"\r?\n|\r").unwrap();
        if Path::new("./state.dat").exists() {
            match fs::read_to_string("./state.dat") {
                Ok(v) => {
                    re.replace_all(v.as_str(), "").parse::<i64>().unwrap() + 1
                }
                Err(_) => 1
            }
        } else {
            1
        }
    }
    pub fn set_start_block(&self, block: i64) -> bool {
        match fs::write("./state.dat", block.to_string()) {
            Ok(_) => true,
            _ => false
        }
    }
}
