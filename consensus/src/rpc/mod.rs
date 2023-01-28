pub mod mock_rpc;
pub mod nimbus_rpc;

use async_trait::async_trait;
use eyre::Result;

use crate::types::{BeaconBlock, Bootstrap, FinalityUpdate, Header, OptimisticUpdate, Update};

// implements https://github.com/ethereum/beacon-APIs/tree/master/apis/beacon/light_client
#[async_trait]
pub trait ConsensusRpc {
    fn new(path: &str) -> Self;
    async fn get_bootstrap(&self, block_root: &'_ [u8]) -> Result<Bootstrap>;
    async fn get_updates(&self, period: u64, count: u8) -> Result<Vec<Update>>;
    async fn get_finality_update(&self) -> Result<FinalityUpdate>;
    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate>;
    async fn get_block(&self, slot: u64) -> Result<BeaconBlock>;
    async fn get_header(&self, slot: u64) -> Result<Header>;
}

#[allow(non_snake_case)]
pub(self) mod HeaderResponse {
    use crate::types::Header;

    #[derive(serde::Deserialize, Debug)]
    pub struct Message {
        pub message: Header,
        pub signature: String,
    }

    #[derive(serde::Deserialize, Debug)]
    pub struct Data {
        pub root: String,
        pub canonical: bool,
        pub header: Message,
    }

    #[derive(serde::Deserialize, Debug)]
    pub struct Response {
        pub execution_optimistic: bool,
        pub data: Data,
    }
}
