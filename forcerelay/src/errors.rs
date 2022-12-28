use ckb_types::H256;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ForcerelayCkbError {
    #[error("invalid ckb rpc url: {0}")]
    InvalidRpcUrl(String),
    #[error("invalid ckb rpc response: {0}")]
    InvalidRpcResponse(String),
    #[error("invalid ethereum 2.0 lightclient contract type_args: {0}")]
    InvalidLightclientContract(H256),
}
