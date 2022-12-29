use ckb_sdk::constants::TYPE_ID_CODE_HASH;
use ckb_sdk::rpc::ckb_indexer::{Cell as IndexerCell, SearchKey};
use ckb_sdk::traits::{CellQueryOptions, PrimaryScriptType};
use ckb_types::core::{ScriptHashType, TransactionView};
use ckb_types::packed::Script;
use ckb_types::prelude::*;
use consensus::types::{BeaconBlock, Header};
use ethers::types::{Transaction, TransactionReceipt};
use eyre::Result;
use reqwest::Url;

use crate::rpc::RpcClient;

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
    ) -> Result<TransactionView> {
        let celldep_contract_cell =
            search_cell(&self.ckb_rpc, &self.contract_typeid_script).await?;
        let lightclient_cell = search_cell(&self.ckb_rpc, &self.lightclient_typescript).await?;
        todo!()
    }
}

async fn search_cell(rpc: &RpcClient, typescript: &Script) -> Result<Option<IndexerCell>> {
    let search: SearchKey =
        CellQueryOptions::new(typescript.clone(), PrimaryScriptType::Type).into();
    let result = rpc.fetch_live_cells(search, 1, None).await?;
    Ok(result.objects.first().cloned())
}
