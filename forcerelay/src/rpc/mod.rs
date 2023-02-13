mod ckb_rpc;
mod rpc_trait;

pub use ckb_rpc::RpcClient;
pub use rpc_trait::CkbRpc;

#[cfg(test)]
mod mock_rpc;
#[cfg(test)]
pub use mock_rpc::*;
