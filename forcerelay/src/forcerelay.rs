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
        client_id: &str,
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

#[cfg(test)]
mod test {
    use ckb_testtool::context::Context;
    use ethers::types::{Transaction, TransactionReceipt};
    use eyre::Result;
    use std::{cell::RefCell, path::PathBuf, sync::Arc};
    use tempfile::TempDir;

    use config::{networks, Config};
    use consensus::types::{BeaconBlock, Header};
    use consensus::{rpc::mock_rpc::MockRpc, ConsensusClient};

    use crate::forcerelay::ForcerelayClient;
    use crate::rpc::{MockRpcClient, BINARY_TYPEID_ARGS, CONTRACT_TYPEID_ARGS, TESTDATA_DIR};
    use crate::setup_test_logger;

    async fn make_consensus(path: PathBuf, last_header: &Header) -> ConsensusClient<MockRpc> {
        let base_config = networks::goerli();
        let config = Config {
            consensus_rpc: String::new(),
            execution_rpc: String::new(),
            chain: base_config.chain,
            forks: base_config.forks,
            max_checkpoint_age: u64::MAX,
            storage_path: path,
            ..Default::default()
        };

        let checkpoint =
            hex::decode("7bc1d0c64d28d63ddd8150bc92bff771011acbddfa314b8288b3fe0a7b3a01cb")
                .unwrap();

        let mut client = ConsensusClient::new(TESTDATA_DIR, &checkpoint, Arc::new(config)).unwrap();
        client.bootstrap(5763680).await.expect("bootstrap");
        client
            .store_updates_until_finality_update(&last_header.clone().into())
            .await
            .expect("until store");
        client
    }

    fn load_json_testdata<'a, V: serde::Deserialize<'a>>(filename: &str) -> Result<V> {
        let contents = std::fs::read(format!("{TESTDATA_DIR}{filename}"))?;
        let static_ref = Box::leak(Box::new(contents));
        let value: V = serde_json::from_slice(static_ref)?;
        Ok(value)
    }

    #[tokio::test]
    async fn test_assemble_tx() {
        setup_test_logger();
        let context = Arc::new(RefCell::new(Context::default()));
        let rpc = MockRpcClient::new(context.clone());
        let forcerelay = ForcerelayClient::new(
            rpc,
            &CONTRACT_TYPEID_ARGS.to_vec(),
            &BINARY_TYPEID_ARGS.to_vec(),
            "client_id",
        );

        let path = TempDir::new().unwrap();
        let headers: Vec<Header> = load_json_testdata("headers.json").expect("load headers");
        let consensus = make_consensus(path.into_path(), headers.last().unwrap()).await;
        let block: BeaconBlock = load_json_testdata("block.json").expect("load block");
        let tx: Transaction = load_json_testdata("transaction.json").expect("load transaction");
        let all_receipts: Vec<TransactionReceipt> =
            load_json_testdata("receipts.json").expect("load receipts");
        let receipt = all_receipts
            .iter()
            .find(|receipt| receipt.transaction_hash == tx.hash)
            .cloned()
            .expect("receipt not found");

        let verifier_tx = forcerelay
            .assemble_tx(&consensus, &block, &tx, &receipt, &all_receipts)
            .await
            .expect("assemble");
        println!("tx = {verifier_tx:?}");
    }
}
