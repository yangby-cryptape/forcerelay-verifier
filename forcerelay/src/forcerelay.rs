use ckb_types::core::TransactionView;
use consensus::rpc::ConsensusRpc;
use consensus::types::BeaconBlock;
use consensus::ConsensusClient;
use eth_light_client_in_ckb_verification::types::{
    core::Client as OnChainClient, prelude::Unpack as _,
};
use ethers::types::{Transaction, TransactionReceipt};
use eyre::Result;
use log::debug;

use crate::assembler::ForcerelayAssembler;
use crate::rpc::CkbRpc;

pub struct ForcerelayClient<R: CkbRpc> {
    assembler: ForcerelayAssembler<R>,
}

impl<R: CkbRpc> ForcerelayClient<R> {
    pub fn new(
        rpc: R,
        contract_typeargs: &Vec<u8>,
        binary_typeargs: &Vec<u8>,
        client_id: &String,
    ) -> Self {
        let assembler =
            ForcerelayAssembler::new(rpc, contract_typeargs, binary_typeargs, client_id);
        Self { assembler }
    }

    pub async fn onchain_client(&self) -> Result<OnChainClient> {
        if let Some(packed_client) = self.assembler.fetch_onchain_packed_client().await? {
            let client = packed_client.unpack();
            debug!("current onchain client {client}");
            Ok(client)
        } else {
            Err(eyre::eyre!("no lightclient cell deployed on ckb"))
        }
    }

    pub async fn assemble_tx(
        &self,
        consensus: &ConsensusClient<impl ConsensusRpc>,
        block: &BeaconBlock,
        tx: &Transaction,
        receipt: &TransactionReceipt,
        all_receipts: &[TransactionReceipt],
    ) -> Result<TransactionView> {
        self.assembler
            .assemble_tx(consensus, block, tx, receipt, all_receipts)
            .await
    }
}

#[tokio::test]
async fn test_assemble_tx() {
    use crate::rpc::MockRpcClient;

    const CONTRACT_TYPEID_ARGS: [u8; 32] = [0u8; 32];
    const BINARY_TYPEID_ARGS: [u8; 32] = [1u8; 32];

    let rpc = MockRpcClient::default();
    let client_id = "client_id".to_owned();
    let forcerelay = ForcerelayClient::new(
        rpc,
        &CONTRACT_TYPEID_ARGS.to_vec(),
        &BINARY_TYPEID_ARGS.to_vec(),
        &client_id,
    );
}
