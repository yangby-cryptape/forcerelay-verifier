use async_trait::async_trait;
use common::errors::RpcError;
use eth2_types::{EthSpec, MainnetEthSpec};
use eyre::Result;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::cmp;

use super::{ConsensusRpc, HeaderResponse};
use crate::constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES;
use crate::types::*;

pub struct NimbusRpc {
    rpc: String,
    client: ClientWithMiddleware,
}

#[async_trait]
impl ConsensusRpc for NimbusRpc {
    fn new(rpc: &str) -> Self {
        let retry_policy = ExponentialBackoff::builder()
            .backoff_exponent(1)
            .build_with_max_retries(3);

        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        NimbusRpc {
            rpc: rpc.to_string(),
            client,
        }
    }

    async fn get_bootstrap(&self, block_root: &'_ [u8]) -> Result<Bootstrap> {
        let root_hex = hex::encode(block_root);
        let req = format!(
            "{}/eth/v1/beacon/light_client/bootstrap/0x{}",
            self.rpc, root_hex
        );

        let res = self
            .client
            .get(req)
            .send()
            .await
            .map_err(|e| RpcError::new("bootstrap", e))?
            .json::<BootstrapResponse>()
            .await
            .map_err(|e| RpcError::new("bootstrap", e))?;

        Ok(res.data)
    }

    async fn get_updates(&self, period: u64, count: u8) -> Result<Vec<Update>> {
        let count = cmp::min(count, MAX_REQUEST_LIGHT_CLIENT_UPDATES);
        let req = format!(
            "{}/eth/v1/beacon/light_client/updates?start_period={}&count={}",
            self.rpc, period, count
        );

        let res = self
            .client
            .get(req)
            .send()
            .await
            .map_err(|e| RpcError::new("updates", e))?
            .json::<UpdateResponse>()
            .await
            .map_err(|e| RpcError::new("updates", e))?;

        Ok(res.iter().map(|d| d.data.clone()).collect())
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        let req = format!("{}/eth/v1/beacon/light_client/finality_update", self.rpc);
        let res = self
            .client
            .get(req)
            .send()
            .await
            .map_err(|e| RpcError::new("finality_update", e))?
            .json::<FinalityUpdateResponse>()
            .await
            .map_err(|e| RpcError::new("finality_update", e))?;

        Ok(res.data)
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate> {
        let req = format!("{}/eth/v1/beacon/light_client/optimistic_update", self.rpc);
        let res = self
            .client
            .get(req)
            .send()
            .await
            .map_err(|e| RpcError::new("optimistic_update", e))?
            .json::<OptimisticUpdateResponse>()
            .await
            .map_err(|e| RpcError::new("optimistic_update", e))?;

        Ok(res.data)
    }

    async fn get_block(&self, slot: u64) -> Result<Option<BeaconBlock>> {
        let req = format!("{}/eth/v2/beacon/blocks/{}", self.rpc, slot);
        let res = self
            .client
            .get(req.clone())
            .send()
            .await
            .map_err(|e| RpcError::new(req.as_str(), e))?
            .json::<BeaconBlockResponse>()
            .await
            .map_err(|e| RpcError::new(format!("blocks {slot}").as_str(), e))?;

        Ok(res.block())
    }

    async fn get_block_ssz(&self, slot: u64) -> Result<Option<BeaconBlock>> {
        let req = format!("{}/eth/v2/beacon/blocks/{}", self.rpc, slot);
        let ssz_res = self
            .client
            .get(req.clone())
            .header("accept", "application/octet-stream")
            .send()
            .await
            .map_err(|e| RpcError::new(req.as_str(), e))?
            .bytes()
            .await
            .map_err(|e| RpcError::new(format!("blocks {slot}").as_str(), e))?;
        match serde_json::from_slice::<BeaconBlockResponse>(&ssz_res) {
            Ok(value) => Ok(value.block()),
            Err(_) => {
                let signed_block =
                    SignedBeaconBlock::from_ssz_bytes(&ssz_res, &MainnetEthSpec::default_spec())
                        .map_err(|e| RpcError::new("blocks_ssz", format!("{e:?}")))?;
                let (block, _) = signed_block.deconstruct();
                Ok(Some(block))
            }
        }
    }

    async fn get_header(&self, slot: u64) -> Result<Option<Header>> {
        let req = format!("{}/eth/v1/beacon/headers/{}", self.rpc, slot);
        let res = self
            .client
            .get(req)
            .send()
            .await?
            .json::<HeaderResponse::Response>()
            .await
            .map_err(|e| eyre::eyre!(format!("{e} (slot {slot})")))?;

        Ok(res.header())
    }
}

#[allow(unused)]
#[derive(serde::Deserialize, Debug)]
struct BeaconBlockResponse {
    data: Option<BeaconBlockData>,
    code: Option<u64>,
    message: Option<String>,
}

impl BeaconBlockResponse {
    pub fn block(self) -> Option<BeaconBlock> {
        if let Some(data) = self.data {
            Some(data.message)
        } else {
            None
        }
    }
}

#[derive(serde::Deserialize, Debug)]
struct BeaconBlockData {
    message: BeaconBlock,
}

type UpdateResponse = Vec<UpdateData>;

#[derive(serde::Deserialize, Debug)]
struct UpdateData {
    data: Update,
}

#[derive(serde::Deserialize, Debug)]
struct FinalityUpdateResponse {
    data: FinalityUpdate,
}

#[derive(serde::Deserialize, Debug)]
struct OptimisticUpdateResponse {
    data: OptimisticUpdate,
}

#[derive(serde::Deserialize, Debug)]
struct BootstrapResponse {
    data: Bootstrap,
}
