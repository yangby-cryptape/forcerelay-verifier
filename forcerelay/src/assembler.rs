use ckb_sdk::constants::TYPE_ID_CODE_HASH;
use ckb_types::core::{ScriptHashType, TransactionView};
use ckb_types::packed::{CellDep, Script};
use ckb_types::prelude::*;
use consensus::rpc::ConsensusRpc;
use consensus::types::BeaconBlock;
use consensus::ConsensusClient;
use eth2_types::MainnetEthSpec;
use eth_light_client_in_ckb_prover::{CachedBeaconBlock, Receipts};
use eth_light_client_in_ckb_verification::mmr;
use eth_light_client_in_ckb_verification::types::{core, packed, prelude::Unpack as LcUnpack};
use ethers::types::{Transaction, TransactionReceipt};
use eyre::Result;
use storage::prelude::StorageAsMMRStore as _;

use crate::rpc::CkbRpc;
use crate::util::*;

pub struct ForcerelayAssembler<R: CkbRpc> {
    rpc: R,
    last_maximal_slot: u64,
    last_header_mmr_proof: Vec<core::HeaderDigest>,
    binary_celldep: CellDep,
    pub binary_typeid_script: Script,
    pub lightclient_typescript: Script,
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
            last_maximal_slot: 0,
            last_header_mmr_proof: vec![],
            binary_celldep: CellDep::default(),
            binary_typeid_script,
            lightclient_typescript,
        }
    }

    pub async fn fetch_onchain_packed_client(&self) -> Result<Option<(core::Client, CellDep)>> {
        match search_cell(&self.rpc, &self.lightclient_typescript).await? {
            Some(cell) => {
                if packed::ClientReader::verify(&cell.output_data, false).is_err() {
                    return Err(eyre::eyre!("unsupported lightlient data"));
                }
                let packed_client = packed::Client::new_unchecked(cell.output_data);
                let celldep = CellDep::new_builder().out_point(cell.out_point).build();
                Ok(Some((packed_client.unpack(), celldep)))
            }
            None => Ok(None),
        }
    }

    pub async fn update_binary_celldep(&mut self) -> Result<()> {
        if let Some(binary_celldep) =
            search_cell_as_celldep(&self.rpc, &self.binary_typeid_script).await?
        {
            self.binary_celldep = binary_celldep;
            Ok(())
        } else {
            Err(eyre::eyre!("light client binary cell not found"))
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn assemble_tx(
        &mut self,
        client: &core::Client,
        client_celldep: &CellDep,
        consensus: &ConsensusClient<impl ConsensusRpc>,
        block: &BeaconBlock,
        tx: &Transaction,
        receipt: &TransactionReceipt,
        all_receipts: &[TransactionReceipt],
    ) -> Result<TransactionView> {
        let block: CachedBeaconBlock<MainnetEthSpec> = block.clone().into();
        let receipts: Receipts = all_receipts.to_owned().into();

        if self.last_maximal_slot != client.maximal_slot {
            let mmr = consensus.storage().chain_root_mmr(client.maximal_slot)?;
            let mmr_position = block.slot() - client.minimal_slot;
            let mmr_index = mmr::lib::leaf_index_to_pos(mmr_position.into());
            self.last_header_mmr_proof = mmr
                .gen_proof(vec![mmr_index])?
                .proof_items()
                .iter()
                .map(LcUnpack::unpack)
                .collect();
            self.last_maximal_slot = client.maximal_slot;
        }

        let celldeps = vec![self.binary_celldep.clone(), client_celldep.clone()];
        assemble_partial_verification_transaction(
            &block,
            tx,
            receipt,
            &receipts,
            &celldeps,
            client,
            &self.last_header_mmr_proof,
        )
    }
}
