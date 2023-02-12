use ckb_jsonrpc_types::{
    BlockNumber, BlockView, CellWithStatus, HeaderView, JsonBytes, OutPoint, OutputsValidator,
    Transaction, TransactionWithStatus,
};
use ckb_sdk::rpc::ckb_indexer::{Cell, Pagination, SearchKey};
use ckb_types::H256;
use std::{future::Future, pin::Pin};

use crate::errors::ForcerelayCkbError;

pub type Rpc<T> = Pin<Box<dyn Future<Output = Result<T, ForcerelayCkbError>> + Send + 'static>>;

pub trait CkbRpc {
    fn get_block_by_number(&self, number: BlockNumber) -> Rpc<BlockView>;

    fn get_block(&self, hash: &H256) -> Rpc<BlockView>;

    fn get_tip_header(&self) -> Rpc<HeaderView>;

    fn get_transaction(&self, hash: &H256) -> Rpc<Option<TransactionWithStatus>>;

    fn get_live_cell(&self, out_point: &OutPoint, with_data: bool) -> Rpc<CellWithStatus>;

    fn get_txs_by_hashes(&self, hashes: Vec<H256>) -> Rpc<Vec<Option<TransactionWithStatus>>>;

    fn fetch_live_cells(
        &self,
        search_key: SearchKey,
        limit: u32,
        cursor: Option<JsonBytes>,
    ) -> Rpc<Pagination<Cell>>;

    fn send_transaction(
        &self,
        tx: &Transaction,
        outputs_validator: Option<OutputsValidator>,
    ) -> Rpc<H256>;
}
