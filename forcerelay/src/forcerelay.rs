use ckb_types::core::TransactionView;
use consensus::rpc::ConsensusRpc;
use consensus::types::{BeaconBlock, Header};
use consensus::ConsensusClient;
use eth_light_client_in_ckb_verification::types::prelude::Unpack;
use ethers::types::{Transaction, TransactionReceipt};
use eyre::Result;
use reqwest::Url;

use crate::assembler::ForcerelayAssembler;
use crate::rpc::RpcClient;

pub struct ForcerelayClient {
    assembler: ForcerelayAssembler,
    // TODO: add mmr storage variable here
}

impl ForcerelayClient {
    pub fn new(
        rpc_url: &str,
        storage_path: &str,
        contract_typeargs: &Vec<u8>,
        binary_typeargs: &Vec<u8>,
        client_id: &String,
    ) -> Self {
        let url = Url::parse(rpc_url).expect("parse ckb url");
        let rpc = RpcClient::new(&url, &url);
        let assembler =
            ForcerelayAssembler::new(rpc, contract_typeargs, binary_typeargs, client_id);
        Self { assembler }
    }

    pub async fn bootstrap(&self, consensus: &ConsensusClient<impl ConsensusRpc>) -> Result<()> {
        self.align_headers(consensus).await?;
        Ok(())
    }

    pub async fn align_headers(
        &self,
        consensus: &ConsensusClient<impl ConsensusRpc>,
    ) -> Result<()> {
        if let Some(packed_client) = self.assembler.fetch_onchain_packed_client().await? {
            let minimal_header_slot: u64 = packed_client.minimal_slot().unpack();
            let maximal_header_slot: u64 = packed_client.maximal_slot().unpack();
            // TODO: align headers from native and onchain
        } else {
            return Err(eyre::eyre!("no lightclient cell deployed on ckb"));
        }
        Ok(())
    }

    pub async fn assemble_tx(
        &self,
        block: &BeaconBlock,
        tx: &Transaction,
        receipt: &TransactionReceipt,
        all_receipts: &[TransactionReceipt],
    ) -> Result<TransactionView> {
        // TODO: replace `headers` with mmr variable here
        let headers = vec![];
        self.assembler
            .assemble_tx(&headers, block, tx, receipt, all_receipts)
            .await
    }
}
