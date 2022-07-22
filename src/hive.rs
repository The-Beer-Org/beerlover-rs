use std::iter::Iterator;
use serde_json::{json, Value};
use reqwest::{Client};
use crate::StakingQueueAction;

pub mod hive_ops;

#[derive(Debug, Clone)]
pub struct HivePost {
    pub author: String,
    pub permlink: String,
    pub parent_author: String,
    pub parent_permlink: String,
    pub body: String,
    pub tx_id: String,
    pub action: StakingQueueAction
}

pub type HivePostList = Vec<HivePost>;

impl HivePost {
    pub fn from(op: Value, tx_id: String, action: StakingQueueAction) -> Self {
        HivePost {
            author: op[1]["author"].as_str().unwrap().to_string(),
            permlink: op[1]["permlink"].as_str().unwrap().to_string(),
            parent_author: op[1]["parent_author"].as_str().unwrap().to_string(),
            parent_permlink: op[1]["parent_permlink"].as_str().unwrap().to_string(),
            body: op[1]["body"].as_str().unwrap().to_string(),
            tx_id,
            action
        }
    }
}

pub struct Counter {
    count: i64,
}

impl Counter {
    pub fn new(start: i64) -> Counter {
        Counter {
            count: start
        }
    }
    pub fn next(&mut self) -> i64 {
        let cur = self.count;
        self.count = self.count.clone().to_owned() + 1i64;
        cur.to_owned()
    }
}

pub struct Hive {
    rpc_host: String,
    http_client: Client,
    request_id_generator: Counter,
}

impl Hive {
    async fn request(&self, body: Vec<u8>) -> Value {
        let result = &self.http_client
            .post(&self.rpc_host)
            .header("Content-Type", "application/json")
            .body(reqwest::Body::from(body)).send()
            .await.unwrap().json::<Value>().await;

        match result {
            Ok(r) => r.to_owned(),
            Err(_) => panic!("Failed to request Hive RPC!")
        }
    }

    pub fn new(rpc_host: String, http_client: Client, request_id_generator: Counter) -> Hive {
        Hive {
            rpc_host,
            http_client,
            request_id_generator,
        }
    }

    pub async fn get_ignore_list(&mut self, account: String) -> Vec<String> {
        let request_id = self.request_id_generator.next();

        let body = json!({
            "id": request_id,
            "jsonrpc":"2.0",
            "method":"condenser_api.get_following",
            "params":[
                account,
                "",
                "ignore",
                250
            ]
        });

        let request_body = serde_json::to_string(&body).unwrap().as_bytes().to_vec();

        let result = self.request(request_body).await;

        if result["id"] != request_id {
            panic!("Request ID does not match! Expected {} got {}", request_id, result["id"]);
        }

        let mut ignore_list: Vec<String> = vec![];
        let result = result["result"].as_array().unwrap().iter();

        for account in result {
            ignore_list.push(account["following"].as_str().unwrap().to_string());
        }

        ignore_list
    }

    pub async fn get_head_block(&mut self) -> i64 {
        let request_id = self.request_id_generator.next();

        let body = json!({
            "id": request_id,
            "jsonrpc":"2.0",
            "method":"condenser_api.get_dynamic_global_properties",
            "params":[]
        });

        let request_body = serde_json::to_string(&body).unwrap().as_bytes().to_vec();

        let result = self.request(request_body).await;

        if result["id"] != request_id {
            panic!("Request ID does not match! Expected {} got {}", request_id, result["id"]);
        }

        result["result"]["head_block_number"].as_i64().unwrap()

    }

    pub async fn get_block(&mut self, block: i64) -> Value {
        let request_id = self.request_id_generator.next();

        let body = json!({
            "id": request_id,
            "jsonrpc": "2.0",
            "method": "condenser_api.get_block",
            "params": [block]
        });

        let request_body = serde_json::to_string(&body).unwrap().as_bytes().to_vec();

        let result = self.request(request_body).await;

        if result["id"] != request_id {
            panic!("Request ID does not match! Expected {} got {}", request_id, result["id"]);
        }

        result
    }
}

pub struct HiveEngine {
    rpc_host: String,
    http_client: Client,
    request_id_generator: Counter,
}

impl HiveEngine {
    async fn request(&self, body: Vec<u8>) -> Value {
        let result = &self.http_client
            .post(&self.rpc_host)
            .header("Content-Type", "application/json")
            .body(reqwest::Body::from(body)).send()
            .await.unwrap().json::<Value>().await;

        match result {
            Ok(r) => r.to_owned(),
            Err(_) => panic!("Failed to request Hive Engine RPC!")
        }
    }

    pub fn new(rpc_host: String, http_client: Client, request_id_generator: Counter) -> HiveEngine {
        HiveEngine {
            rpc_host,
            http_client,
            request_id_generator,
        }
    }

    pub async fn balance(&mut self, account: String, token: String) -> f64 {
        let request_id = self.request_id_generator.next();

        let body = json!({
            "id": request_id,
            "jsonrpc": "2.0",
            "method": "find",
            "params": {
                "contract": "tokens",
                "query": {
                    "account": account,
                    "symbol": token
                },
                "table": "balances"
            }
        });

        let request_body = serde_json::to_string(&body).unwrap().as_bytes().to_vec();

        let mut result = self.request(request_body).await;

        if result["id"] != request_id {
            panic!("Request ID does not match! Expected {} got {}", request_id, result["id"]);
        }

        match result["result"].as_array_mut().unwrap().iter().find(|b| b.to_owned()["symbol"].as_str().unwrap().to_string().contains("BEER")) {
            Some(b) => b["balance"].as_str().unwrap().parse::<f64>().unwrap(),
            None => 0f64
        }
    }
    pub async fn stake(&mut self, account: String, token: String) -> f64 {
        let request_id = self.request_id_generator.next();

        let body = json!({
            "id": request_id,
            "jsonrpc": "2.0",
            "method": "find",
            "params": {
                "contract": "tokens",
                "query": {
                    "account": account,
                    "symbol": token
                },
                "table": "balances"
            }
        });

        let request_body = serde_json::to_string(&body).unwrap().as_bytes().to_vec();

        let mut result = self.request(request_body).await;

        if result["id"] != request_id {
            panic!("Request ID does not match! Expected {} got {}", request_id, result["id"]);
        }

        match result["result"].as_array_mut().unwrap().iter().find(|b| b.to_owned()["symbol"].as_str().unwrap().to_string().contains("BEER")) {
            Some(b) => b["stake"].as_str().unwrap().parse::<f64>().unwrap(),
            None => 0f64
        }
    }
}
