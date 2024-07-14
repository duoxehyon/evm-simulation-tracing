use std::collections::HashMap;
use std::thread;
use std::error::Error;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{channel, Sender, Receiver};
use tokio::sync::oneshot;
use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    primitives::{Address, B256, U256},
    providers::{Provider, RootProvider},
    rpc::types::BlockTransactionsKind,
    transports::http::{Client, Http},
};

#[derive(Clone)]
pub struct AccountInfo {
    pub balance: U256,
    pub nonce: u64,
    pub code: Vec<u8>,
}

enum BackendRequest {
    GetAccount(Address),
    GetStorageAt(Address, B256),
    GetBlockHash(U256),
}

enum BackendResponse {
    Account(Result<AccountInfo, Box<dyn Error + Send + Sync>>),
    Storage(Result<U256, Box<dyn Error + Send + Sync>>),
    BlockHash(Result<B256, Box<dyn Error + Send + Sync>>),
}

pub struct Backend {
    sender: Sender<(BackendRequest, oneshot::Sender<BackendResponse>)>,
}

impl Backend {
    pub fn new(provider: RootProvider<Http<Client>>) -> Self {
        let (sender, receiver) = channel(100);
        let backend = Self { sender };

        thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let mut db = HashMap::new();
                backend_loop(provider, receiver, &mut db).await;
            });
        });

        backend
    }

    pub fn get_account(&self, address: Address) -> Result<AccountInfo, Box<dyn Error + Send + Sync>> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.sender.blocking_send((BackendRequest::GetAccount(address), response_sender)).unwrap();
        match response_receiver.blocking_recv().unwrap() {
            BackendResponse::Account(result) => result,
            _ => unreachable!(),
        }
    }

    pub fn get_storage_at(&self, address: Address, slot: B256) -> Result<U256, Box<dyn Error + Send + Sync>> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.sender.blocking_send((BackendRequest::GetStorageAt(address, slot), response_sender)).unwrap();
        match response_receiver.blocking_recv().unwrap() {
            BackendResponse::Storage(result) => result,
            _ => unreachable!(),
        }
    }

    pub fn get_block_hash(&self, number: U256) -> Result<B256, Box<dyn Error + Send + Sync>> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.sender.blocking_send((BackendRequest::GetBlockHash(number), response_sender)).unwrap();
        match response_receiver.blocking_recv().unwrap() {
            BackendResponse::BlockHash(result) => result,
            _ => unreachable!(),
        }
    }
}

async fn backend_loop(
    provider: RootProvider<Http<Client>>,
    mut receiver: Receiver<(BackendRequest, oneshot::Sender<BackendResponse>)>,
    db: &mut HashMap<Address, AccountInfo>,
) {
    while let Some((request, response_sender)) = receiver.recv().await {
        match request {
            BackendRequest::GetAccount(address) => {
                let result = get_account(&provider, db, address).await;
                let _ = response_sender.send(BackendResponse::Account(result));
            }
            BackendRequest::GetStorageAt(address, slot) => {
                let result = get_storage_at(&provider, address, slot).await;
                let _ = response_sender.send(BackendResponse::Storage(result));
            }
            BackendRequest::GetBlockHash(number) => {
                let result = get_block_hash(&provider, number).await;
                let _ = response_sender.send(BackendResponse::BlockHash(result));
            }
        }
    }
}

async fn get_account(
    provider: &RootProvider<Http<Client>>,
    db: &mut HashMap<Address, AccountInfo>,
    address: Address,
) -> Result<AccountInfo, Box<dyn Error + Send + Sync>> {
    if let Some(account) = db.get(&address) {
        return Ok(account.clone());
    }

    let balance = provider.get_balance(address).await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
    let nonce = provider.get_transaction_count(address).await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
    let code = provider.get_code_at(address).await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

    let account_info = AccountInfo {
        balance,
        nonce,
        code: code.to_vec(),
    };

    db.insert(address, account_info.clone());
    Ok(account_info)
}

async fn get_storage_at(
    provider: &RootProvider<Http<Client>>,
    address: Address,
    slot: B256,
) -> Result<U256, Box<dyn Error + Send + Sync>> {
    provider.get_storage_at(address, slot.into()).await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
}

async fn get_block_hash(
    provider: &RootProvider<Http<Client>>,
    number: U256,
) -> Result<B256, Box<dyn Error + Send + Sync>> {
    let block = provider.get_block(
        BlockId::Number(BlockNumberOrTag::Number(number.to::<u64>())),
        BlockTransactionsKind::Hashes
    ).await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
    block.ok_or_else(|| Box::<dyn Error + Send + Sync>::from("Block not found")).map(|b| b.header.hash.unwrap_or_default())
}