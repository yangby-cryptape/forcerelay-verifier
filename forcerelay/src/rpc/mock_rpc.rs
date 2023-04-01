use std::cell::{RefCell, RefMut};
use std::sync::Arc;

use ckb_jsonrpc_types::{
    BlockNumber, BlockView, CellWithStatus, HeaderView, JsonBytes, OutPoint, OutputsValidator,
    Transaction, TransactionWithStatus,
};
use ckb_sdk::constants::TYPE_ID_CODE_HASH;
use ckb_sdk::rpc::ckb_indexer::{Cell, Pagination, SearchKey};
use ckb_types::core::ScriptHashType;
use ckb_types::packed::Script;
use ckb_types::{prelude::*, H256};
use test_utils::Context;

use crate::rpc::rpc_trait::{CkbRpc, Rpc};

pub const CONTRACT_TYPEID_ARGS: [u8; 32] = [0u8; 32];
pub const BINARY_TYPEID_ARGS: [u8; 32] = [1u8; 32];
pub const TESTDATA_DIR: &str = "testdata/";

const VERIFY_BIN: &str = "eth_light_client-verify_bin";
const PACKED_CLIENT_5763680_TO_5763839: &str = "packed_client";

pub struct MockRpcClient {
    context: Arc<RefCell<Context>>,
    lightclient_typeid: H256,
}

impl MockRpcClient {
    pub fn new(context: Arc<RefCell<Context>>) -> Self {
        let contract_script = Script::new_builder()
            .code_hash(TYPE_ID_CODE_HASH.0.pack())
            .args(CONTRACT_TYPEID_ARGS.to_vec().pack())
            .hash_type(ScriptHashType::Type.into())
            .build();
        let lightclient_typeid = contract_script.calc_script_hash().unpack();
        Self {
            context,
            lightclient_typeid,
        }
    }

    fn mut_context(&self) -> RefMut<'_, Context> {
        self.context.borrow_mut()
    }
}

impl CkbRpc for MockRpcClient {
    fn get_block_by_number(&self, _number: BlockNumber) -> Rpc<BlockView> {
        unimplemented!()
    }

    fn get_block(&self, _hash: &H256) -> Rpc<BlockView> {
        unimplemented!()
    }

    fn get_tip_header(&self) -> Rpc<HeaderView> {
        unimplemented!()
    }

    fn get_transaction(&self, _hash: &H256) -> Rpc<Option<TransactionWithStatus>> {
        unimplemented!()
    }

    fn get_live_cell(&self, _out_point: &OutPoint, _with_data: bool) -> Rpc<CellWithStatus> {
        unimplemented!()
    }

    fn send_transaction(
        &self,
        _tx: &Transaction,
        _outputs_validator: Option<OutputsValidator>,
    ) -> Rpc<H256> {
        unimplemented!()
    }

    fn get_txs_by_hashes(&self, _hashes: Vec<H256>) -> Rpc<Vec<Option<TransactionWithStatus>>> {
        unimplemented!()
    }

    fn fetch_live_cells(
        &self,
        search_key: SearchKey,
        _limit: u32,
        _cursor: Option<JsonBytes>,
    ) -> Rpc<Pagination<Cell>> {
        let search_script: Script = search_key.script.into();
        let mut live_cell = Cell {
            output: Default::default(),
            out_point: Default::default(),
            output_data: Default::default(),
            block_number: 0.into(),
            tx_index: 0.into(),
        };
        if search_script.code_hash().as_bytes() == TYPE_ID_CODE_HASH.as_bytes()
            && search_script.args().raw_data() == BINARY_TYPEID_ARGS.to_vec()
        {
            // handle verify binary contract search
            let data =
                std::fs::read(format!("{TESTDATA_DIR}lightclient/{VERIFY_BIN}")).expect("read bin");
            let deployed_cell =
                self.mut_context()
                    .deploy(data.into(), Default::default(), Some(search_script));
            live_cell.out_point = deployed_cell.out_point().into();
        } else if search_script.code_hash() == self.lightclient_typeid.pack() {
            // handle light client cell search
            let data = {
                let data = std::fs::read(format!(
                    "{TESTDATA_DIR}lightclient/{PACKED_CLIENT_5763680_TO_5763839}"
                ))
                .expect("read client");
                hex::decode(data).expect("unhex client")
            };
            let deployed_cell = self.mut_context().deploy(
                data.clone().into(),
                Default::default(),
                Some(search_script),
            );
            live_cell.output_data = JsonBytes::from_vec(data);
            live_cell.out_point = deployed_cell.out_point().into();
        } else {
            panic!("unsupported search_script: {search_script}");
        }

        Box::pin(async {
            let result = Pagination::<Cell> {
                objects: vec![live_cell],
                last_cursor: JsonBytes::default(),
            };
            Ok(result)
        })
    }
}
