use ckb_sdk::constants::TYPE_ID_CODE_HASH;
use ckb_sdk::traits::LiveCell;
use ckb_types::core::{ScriptHashType, TransactionView};
use ckb_types::packed::{CellDep, Script};
use ckb_types::prelude::*;
use consensus::rpc::ConsensusRpc;
use consensus::types::BeaconBlock;
use consensus::ConsensusClient;
use eth2_types::MainnetEthSpec;
use eth_light_client_in_ckb_prover::{CachedBeaconBlock, Receipts};
use eth_light_client_in_ckb_verification::types::{packed, prelude::Unpack};
use ethers::types::{Transaction, TransactionReceipt};
use eyre::Result;

use crate::rpc::CkbRpc;
use crate::util::*;

pub struct ForcerelayAssembler<R: CkbRpc> {
    rpc: R,
    binary_typeid_script: Script,
    lightclient_typescript: Script,
}

impl<R: CkbRpc> ForcerelayAssembler<R> {
    pub fn new(
        rpc: R,
        contract_typeargs: &Vec<u8>,
        binary_typeargs: &Vec<u8>,
        client_id: &str,
    ) -> Self {
        let contract_typeid_script = Script::new_builder()
            .code_hash(TYPE_ID_CODE_HASH.0.pack())
            .args(contract_typeargs.pack())
            .hash_type(ScriptHashType::Type.into())
            .build();
        let binary_typeid_script = Script::new_builder()
            .code_hash(TYPE_ID_CODE_HASH.0.pack())
            .args(binary_typeargs.pack())
            .hash_type(ScriptHashType::Type.into())
            .build();
        let contract_typeid = contract_typeid_script.calc_script_hash();
        let lightclient_typescript = Script::new_builder()
            .code_hash(contract_typeid)
            .args(client_id.as_bytes().to_vec().pack())
            .hash_type(ScriptHashType::Type.into())
            .build();
        Self {
            rpc,
            binary_typeid_script,
            lightclient_typescript,
        }
    }

    pub async fn fetch_onchain_packed_client(&self) -> Result<Option<packed::Client>> {
        match search_cell(&self.rpc, &self.lightclient_typescript).await? {
            Some(cell) => {
                if packed::ClientReader::verify(&cell.output_data, false).is_err() {
                    return Err(eyre::eyre!("unsupported lightlient data"));
                }
                let packed_client = packed::Client::new_unchecked(cell.output_data);
                Ok(Some(packed_client))
            }
            None => Ok(None),
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
        let (binary_celldep, lightclient_cell) = prepare_onchain_data(
            &self.rpc,
            &self.binary_typeid_script,
            &self.lightclient_typescript,
        )
        .await?;
        if packed::ClientReader::verify(&lightclient_cell.output_data, false).is_err() {
            return Err(eyre::eyre!("unsupported lightclient data"));
        }
        let client = packed::Client::new_unchecked(lightclient_cell.output_data).unpack();
        log::debug!("current onchain client {client}");
        let block: CachedBeaconBlock<MainnetEthSpec> = block.clone().into();
        let receipts: Receipts = all_receipts.to_owned().into();
        assemble_partial_verification_transaction(
            consensus,
            &block,
            tx,
            receipt,
            &receipts,
            &binary_celldep,
            &client,
        )
    }
}

async fn prepare_onchain_data<R: CkbRpc>(
    rpc: &R,
    binary_script: &Script,
    lightclient_script: &Script,
) -> Result<(CellDep, LiveCell)> {
    let binary_celldep = {
        let celldep_opt = search_cell_as_celldep(rpc, binary_script).await?;
        if celldep_opt.is_none() {
            return Err(eyre::eyre!("lightClient binary not found"));
        }
        celldep_opt.unwrap()
    };
    let lightclient_cell = {
        let cell_opt = search_cell(rpc, lightclient_script).await?;
        if cell_opt.is_none() {
            return Err(eyre::eyre!("lightClient cell not found"));
        }
        cell_opt.unwrap()
    };
    Ok((binary_celldep, lightclient_cell))
}
