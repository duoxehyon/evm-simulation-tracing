use revm::{
    db::{CacheDB, DatabaseRef, EmptyDB},
    primitives::{AccountInfo, Address as rAddress, Bytecode as rBytecode, HashMap as rHashMap, B256 as rB256, KECCAK_EMPTY, U256 as rU256},
    Database, DatabaseCommit,
};
use sha3::{Digest, Keccak256};
use std::{str::FromStr, sync::Arc};
use alloy::{primitives::{Address, B256}, providers::RootProvider, transports::http::{Client, Http}};

use crate::provider::Backend;

pub struct ForkDB {
    db: CacheDB<EmptyDB>,
    provider: Arc<dyn EthProvider>,
}

pub trait EthProvider: Send + Sync {
    fn get_basic(&self, address: rAddress) -> Option<AccountInfo>;
    fn get_storage(&self, address: rAddress, index: rU256) -> rU256;
    fn get_block_hash(&self, number: rU256) -> rB256;
}

impl ForkDB {
    pub fn new(db: CacheDB<EmptyDB>, provider: Arc<dyn EthProvider>) -> Self {
        Self { db, provider }
    }
}

impl Database for ForkDB {
    type Error = Box<dyn std::error::Error>;

    fn basic(&mut self, address: rAddress) -> Result<Option<AccountInfo>, Self::Error> {
        if let Some(account) = self.db.accounts.get(&address) {
            return Ok(Some(account.info.clone()));
        }
        
        let info = self.provider.get_basic(address);
        if let Some(info) = info.clone() {
            self.db.insert_account_info(address, info);
        }
        Ok(info)
    }

    fn storage(&mut self, address: rAddress, index: rU256) -> Result<rU256, Self::Error> {
        if let Some(account) = self.db.accounts.get(&address) {
            if let Some(entry) = account.storage.get(&index) {
                return Ok(*entry);
            }
        }

        let storage_val = self.provider.get_storage(address, index);
        self.db.insert_account_storage(address, index, storage_val).unwrap();
        Ok(storage_val)
    }

    fn block_hash(&mut self, number: u64) -> Result<rB256, Self::Error> {
        let number = rU256::from(number);
        if let Some(hash) = self.db.block_hashes.get(&number) {
            return Ok(*hash);
        }

        let block_hash = self.provider.get_block_hash(number);
        self.db.block_hashes.insert(number, block_hash);
        Ok(block_hash)
    }

    fn code_by_hash(&mut self, code_hash: rB256) -> Result<rBytecode, Self::Error> {
        self.db.code_by_hash(code_hash).map_err(|_| Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Missing code")) as Box<dyn std::error::Error>)
    }
}

impl DatabaseRef for ForkDB {
    type Error = Box<dyn std::error::Error>;

    fn basic_ref(&self, address: rAddress) -> Result<Option<AccountInfo>, Self::Error> {
        if let Some(account) = self.db.accounts.get(&address) {
            Ok(Some(account.info.clone()))
        } else {
            Ok(self.provider.get_basic(address))
        }
    }

    fn storage_ref(&self, address: rAddress, index: rU256) -> Result<rU256, Self::Error> {
        if let Some(account) = self.db.accounts.get(&address) {
            if let Some(entry) = account.storage.get(&index) {
                return Ok(*entry);
            }
        }
        Ok(self.provider.get_storage(address, index))
    }

    fn block_hash_ref(&self, number: u64) -> Result<rB256, Self::Error> {
        if number > u64::MAX {
            return Ok(KECCAK_EMPTY);
        }
        Ok(self.provider.get_block_hash(rU256::from(number)))
    }

    fn code_by_hash_ref(&self, _code_hash: rB256) -> Result<rBytecode, Self::Error> {
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Missing code")) as Box<dyn std::error::Error>)
    }
}

impl DatabaseCommit for ForkDB {
    fn commit(&mut self, changes: rHashMap<rAddress, revm::primitives::Account>) {
        self.db.commit(changes)
    }
}

impl EthProvider for Backend {
    fn get_basic(&self, address: rAddress) -> Option<AccountInfo> {
        let alloy_address = Address::from(address.0);
        self.get_account(alloy_address).ok().map(|acc| {
            let code_hash = rB256::from(keccak256(&acc.code));
            AccountInfo {
                balance: rU256::from_str(&acc.balance.to_string()).unwrap(),
                nonce: acc.nonce,
                code: Some(rBytecode::new_raw(acc.code.into())),
                code_hash,
            }
        })
    }

    fn get_storage(&self, address: rAddress, index: rU256) -> rU256 {
        let alloy_address = Address::from(address.0);
        let alloy_index = B256::from(index.to_be_bytes());
        self.get_storage_at(alloy_address, alloy_index)
            .map(|v| rU256::from_str(&v.to_string()).unwrap())
            .unwrap_or_default()
    }

    fn get_block_hash(&self, number: rU256) -> rB256 {
        let alloy_number = rU256::from_str(&number.to_string()).unwrap();
        self.get_block_hash(alloy_number)
            .map(|h| rB256::from(h.0))
            .unwrap_or_default()
    }
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}
pub fn create_eth_provider(provider: RootProvider<Http<Client>>) -> Arc<dyn EthProvider> {
    Arc::new(Backend::new(provider))
}