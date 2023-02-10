use ckb_jsonrpc_types::{
    BlockNumber, BlockView, CellWithStatus, HeaderView, JsonBytes, OutPoint, OutputsValidator,
    Transaction, TransactionWithStatusResponse,
};
use ckb_sdk::rpc::ckb_indexer::{Cell, Pagination, SearchKey};
use ckb_types::H256;

use crate::rpc::rpc_trait::{CkbRpc, Rpc};

#[derive(Default)]
pub struct MockRpcClient;

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

    fn get_transaction(&self, _hash: &H256) -> Rpc<Option<TransactionWithStatusResponse>> {
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

    fn get_txs_by_hashes(
        &self,
        _hashes: Vec<H256>,
    ) -> Rpc<Vec<Option<TransactionWithStatusResponse>>> {
        unimplemented!()
    }

    fn fetch_live_cells(
        &self,
        _search_key: SearchKey,
        _limit: u32,
        _cursor: Option<JsonBytes>,
    ) -> Rpc<Pagination<Cell>> {
        unimplemented!()
    }
}
