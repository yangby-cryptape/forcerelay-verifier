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
    async fn get_block(&self, slot: u64) -> Result<Option<BeaconBlock>>;
    async fn get_header(&self, slot: u64) -> Result<Option<Header>>;
}

#[allow(non_snake_case)]
#[allow(unused)]
pub(self) mod HeaderResponse {
    use crate::types::Header;

    #[derive(serde::Deserialize, Debug)]
    pub struct Message {
        message: Header,
        signature: String,
    }

    #[derive(serde::Deserialize, Debug)]
    pub struct Data {
        root: String,
        canonical: bool,
        header: Message,
    }

    #[derive(serde::Deserialize, Debug)]
    pub struct Response {
        execution_optimistic: Option<bool>,
        data: Option<Data>,
        code: Option<u64>,
        message: Option<String>,
    }

    impl Response {
        pub fn header(self) -> Option<Header> {
            if let Some(data) = self.data {
                Some(data.header.message)
            } else {
                None
            }
        }
    }
}
