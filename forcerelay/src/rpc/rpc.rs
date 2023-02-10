use ckb_jsonrpc_types::{
    BlockNumber, BlockView, CellWithStatus, HeaderView, JsonBytes, OutPoint, OutputsValidator,
    Transaction, TransactionWithStatusResponse, Uint32,
};
use ckb_sdk::rpc::ckb_indexer::{Cell, Order, Pagination, SearchKey};
use ckb_types::H256;
use futures::FutureExt;
use reqwest::{Client, Url};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::errors::ForcerelayCkbError;
use crate::rpc::rpc_trait::{CkbRpc, Rpc};

#[allow(clippy::upper_case_acronyms)]
enum Target {
    CKB,
    Indexer,
}

macro_rules! jsonrpc {
    ($method:expr, $id:expr, $self:ident, $return:ty$(, $params:ident$(,)?)*) => {{
        let data = format!(
            r#"{{"id": {}, "jsonrpc": "2.0", "method": "{}", "params": {}}}"#,
            $self.id.load(Ordering::Relaxed),
            $method,
            serde_json::to_value(($($params,)*)).unwrap()
        );
        $self.id.fetch_add(1, Ordering::Relaxed);

        let req_json: serde_json::Value = serde_json::from_str(&data).unwrap();

        let url = match $id {
            Target::CKB => $self.ckb_uri.clone(),
            Target::Indexer => $self.indexer_uri.clone(),
        };
        let c = $self.raw.post(url.clone()).json(&req_json);
        async {
            let resp = c
                .send()
                .await
                .map_err(move |_| ForcerelayCkbError::InvalidRpcUrl(url.to_string()))?;
            let output = resp
                .json::<jsonrpc_core::response::Output>()
                .await
                .map_err(|e| ForcerelayCkbError::InvalidRpcResponse(e.to_string()))?;

            match output {
                jsonrpc_core::response::Output::Success(success) => {
                    Ok(serde_json::from_value::<$return>(success.result).unwrap())
                }
                jsonrpc_core::response::Output::Failure(e) => {
                    Err(ForcerelayCkbError::InvalidRpcResponse(format!("{:?}", e)))
                }
            }
        }
    }}
}

#[derive(Clone)]
pub struct RpcClient {
    raw: Client,
    ckb_uri: Url,
    indexer_uri: Url,
    id: Arc<AtomicU64>,
}

impl RpcClient {
    pub fn new(ckb_uri: &str, indexer_uri: &str) -> Self {
        RpcClient {
            raw: Client::new(),
            ckb_uri: Url::parse(ckb_uri).expect("ckb_uri"),
            indexer_uri: Url::parse(indexer_uri).expect("indexer_uri"),
            id: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl CkbRpc for RpcClient {
    fn get_block_by_number(&self, number: BlockNumber) -> Rpc<BlockView> {
        jsonrpc!("get_block_by_number", Target::CKB, self, BlockView, number).boxed()
    }

    fn get_block(&self, hash: &H256) -> Rpc<BlockView> {
        jsonrpc!("get_block", Target::CKB, self, BlockView, hash).boxed()
    }

    fn get_tip_header(&self) -> Rpc<HeaderView> {
        jsonrpc!("get_tip_header", Target::CKB, self, HeaderView).boxed()
    }

    fn get_transaction(&self, hash: &H256) -> Rpc<Option<TransactionWithStatusResponse>> {
        jsonrpc!(
            "get_transaction",
            Target::CKB,
            self,
            Option<TransactionWithStatusResponse>,
            hash
        )
        .boxed()
    }

    fn get_live_cell(&self, out_point: &OutPoint, with_data: bool) -> Rpc<CellWithStatus> {
        jsonrpc!(
            "get_live_cell",
            Target::CKB,
            self,
            CellWithStatus,
            out_point,
            with_data
        )
        .boxed()
    }

    fn send_transaction(
        &self,
        tx: &Transaction,
        outputs_validator: Option<OutputsValidator>,
    ) -> Rpc<H256> {
        jsonrpc!(
            "send_transaction",
            Target::CKB,
            self,
            H256,
            tx,
            outputs_validator
        )
        .boxed()
    }

    fn get_txs_by_hashes(
        &self,
        hashes: Vec<H256>,
    ) -> Rpc<Vec<Option<TransactionWithStatusResponse>>> {
        let mut list = Vec::with_capacity(hashes.len());
        let mut res = Vec::with_capacity(hashes.len());
        for hash in hashes {
            let task = self.get_transaction(&hash);
            list.push(tokio::spawn(task));
        }
        async {
            for i in list {
                let r = i.await.unwrap()?;
                res.push(r);
            }

            Ok(res)
        }
        .boxed()
    }

    fn fetch_live_cells(
        &self,
        search_key: SearchKey,
        limit: u32,
        cursor: Option<JsonBytes>,
    ) -> Rpc<Pagination<Cell>> {
        let order = Order::Asc;
        let limit = Uint32::from(limit);

        jsonrpc!(
            "get_cells",
            Target::Indexer,
            self,
            Pagination<Cell>,
            search_key,
            order,
            limit,
            cursor,
        )
        .boxed()
    }
}
