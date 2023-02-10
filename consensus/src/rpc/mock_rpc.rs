use std::{collections::BTreeMap, fs::read_to_string, path::PathBuf};

use async_trait::async_trait;
use eyre::Result;

use super::ConsensusRpc;
use crate::types::{BeaconBlock, Bootstrap, FinalityUpdate, Header, OptimisticUpdate, Update};

pub struct MockRpc {
    testdata: PathBuf,
    headers: BTreeMap<u64, Header>,
}

#[async_trait]
impl ConsensusRpc for MockRpc {
    fn new(path: &str) -> Self {
        let testdata = PathBuf::from(path);
        let headers = {
            let value = read_to_string(testdata.join("headers.json")).expect("no headers.json");
            serde_json::from_str(&value).expect("headers jsonify")
        };
        MockRpc { testdata, headers }
    }

    async fn get_bootstrap(&self, _block_root: &'_ [u8]) -> Result<Bootstrap> {
        let bootstrap = read_to_string(self.testdata.join("bootstrap.json"))?;
        Ok(serde_json::from_str(&bootstrap)?)
    }

    async fn get_updates(&self, _period: u64, _count: u8) -> Result<Vec<Update>> {
        let updates = read_to_string(self.testdata.join("updates.json"))?;
        Ok(serde_json::from_str(&updates)?)
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        let finality = read_to_string(self.testdata.join("finality.json"))?;
        Ok(serde_json::from_str(&finality)?)
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate> {
        let optimistic = read_to_string(self.testdata.join("optimistic.json"))?;
        Ok(serde_json::from_str(&optimistic)?)
    }

    async fn get_block(&self, _slot: u64) -> Result<BeaconBlock> {
        let block = read_to_string(self.testdata.join("blocks.json"))?;
        Ok(serde_json::from_str(&block)?)
    }

    async fn get_header(&self, slot: u64) -> Result<Header> {
        let header = self.headers.get(&slot).cloned().unwrap_or_else(|| {
            println!("not found header {slot}, replace with empty");
            Header {
                slot,
                ..Default::default()
            }
        });
        Ok(header)
    }
}
