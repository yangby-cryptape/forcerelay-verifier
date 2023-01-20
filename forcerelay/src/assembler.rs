use ckb_sdk::constants::TYPE_ID_CODE_HASH;
use ckb_types::core::{ScriptHashType, TransactionView};
use ckb_types::packed::Script;
use ckb_types::prelude::*;
use consensus::types::{BeaconBlock, Header};
use eth2_types::MainnetEthSpec;
use eth_light_client_in_ckb_prover::{CachedBeaconBlock, Receipts};
use eth_light_client_in_ckb_verification::types::{packed, prelude::Unpack};
use ethers::types::{Transaction, TransactionReceipt};
use eyre::Result;
use reqwest::Url;

use crate::rpc::RpcClient;
use crate::util::*;

pub struct ForcerelayAssembler {
    ckb_rpc: RpcClient,
    contract_typeid_script: Script,
    lightclient_typescript: Script,
}

impl ForcerelayAssembler {
    pub fn new(ckb_url: &str, lightclient_typeargs: &Vec<u8>, client_id: &String) -> Self {
        let typeid_script = Script::new_builder()
            .code_hash(TYPE_ID_CODE_HASH.0.pack())
            .args(lightclient_typeargs.pack())
            .hash_type(ScriptHashType::Type.into())
            .build();
        let typeid = typeid_script.calc_script_hash();
        let typescript = Script::new_builder()
            .code_hash(typeid)
            .args(client_id.as_bytes().to_vec().pack())
            .hash_type(ScriptHashType::Type.into())
            .build();
        let url = Url::parse(ckb_url).expect("parse ckb url");
        let rpc = RpcClient::new(&url, &url);
        Self {
            ckb_rpc: rpc,
            contract_typeid_script: typeid_script,
            lightclient_typescript: typescript,
        }
    }

    pub async fn assemble_tx(
        &self,
        headers: &Vec<Header>,
        block: &BeaconBlock,
        tx: &Transaction,
        receipt: &TransactionReceipt,
        all_receipts: &Vec<TransactionReceipt>,
    ) -> Result<TransactionView> {
        if headers.is_empty() {
            return Err(eyre::eyre!("empty headers"));
        }
        let contract_celldep = {
            let celldep_opt =
                search_cell_as_celldep(&self.ckb_rpc, &self.contract_typeid_script).await?;
            if celldep_opt.is_none() {
                return Err(eyre::eyre!("lightClient contract not found"));
            }
            celldep_opt.unwrap()
        };
        let lightclient_cell = {
            let cell_opt = search_cell(&self.ckb_rpc, &self.lightclient_typescript).await?;
            if cell_opt.is_none() {
                return Err(eyre::eyre!("lightClient not found"));
            }
            cell_opt.unwrap()
        };
        if packed::ClientReader::verify(&lightclient_cell.output_data, false).is_err() {
            return Err(eyre::eyre!("unsupported lightlient data"));
        }
        let packed_client = packed::Client::new_unchecked(lightclient_cell.output_data);
        if headers[0].slot > packed_client.minimal_slot().unpack() {
            return Err(eyre::eyre!("native slot excessive than onchain"));
        }
        let excessive_slots = packed_client.minimal_slot().unpack() - headers[0].slot;
        let headers = headers
            .iter()
            .skip(excessive_slots as usize)
            .map(header_helios_to_lighthouse)
            .collect::<Vec<_>>();
        let block: CachedBeaconBlock<MainnetEthSpec> = block.clone().into();
        let receipts: Receipts = all_receipts.clone().into();
        assemble_partial_verification_transaction(
            &headers,
            &block,
            tx,
            receipt,
            &receipts,
            &contract_celldep,
        )
    }
}
