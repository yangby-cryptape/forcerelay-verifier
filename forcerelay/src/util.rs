use ckb_sdk::rpc::ckb_indexer::SearchKey;
use ckb_sdk::traits::{CellQueryOptions, LiveCell, PrimaryScriptType};
use ckb_types::core::{DepType, TransactionView};
use ckb_types::packed::{BytesOpt, CellDep, Script, WitnessArgs};
use ckb_types::prelude::Pack as _;
use consensus::types::Header;
use eth2_types::{BeaconBlockHeader, Hash256, MainnetEthSpec};
use eth_light_client_in_ckb_prover::{CachedBeaconBlock, Receipts};
use eth_light_client_in_ckb_verification::types::{core, packed, prelude::*};
use ethers::types::Transaction;
use eyre::Result;

use crate::rpc::CkbRpc;

pub async fn search_cell<R: CkbRpc>(rpc: &R, typescript: &Script) -> Result<Option<LiveCell>> {
    let search: SearchKey =
        CellQueryOptions::new(typescript.clone(), PrimaryScriptType::Type).into();
    let result = rpc.fetch_live_cells(search, 1, None).await?;
    Ok(result.objects.first().cloned().map(Into::into))
}

pub async fn search_cell_as_celldep<R: CkbRpc>(
    rpc: &R,
    typescript: &Script,
) -> Result<Option<CellDep>> {
    let cell = {
        let cell_opt = search_cell(rpc, typescript).await?;
        if cell_opt.is_none() {
            return Ok(None);
        }
        cell_opt.unwrap()
    };
    let celldep = CellDep::new_builder()
        .out_point(cell.out_point)
        .dep_type(DepType::Code.into())
        .build();
    Ok(Some(celldep))
}

pub fn header_helios_to_lighthouse(header: &Header) -> BeaconBlockHeader {
    BeaconBlockHeader {
        slot: header.slot.into(),
        proposer_index: header.proposer_index,
        parent_root: Hash256::from_slice(&header.parent_root),
        state_root: Hash256::from_slice(&header.state_root),
        body_root: Hash256::from_slice(&header.body_root),
    }
}

pub fn find_receipt_index(transaction_hash: Hash256, receipts: &Receipts) -> Option<u64> {
    let mut index = None;
    receipts
        .original()
        .iter()
        .enumerate()
        .for_each(|(i, value)| {
            if value.transaction_hash == transaction_hash {
                index = Some(i as u64);
            }
        });
    index
}

pub fn generate_packed_transaction_proof(
    block: &CachedBeaconBlock<MainnetEthSpec>,
    receipts: &Receipts,
    transaction_index: u64,
    header_mmr_proof: &[core::HeaderDigest],
) -> Result<packed::TransactionProof> {
    let transaction_ssz_proof =
        block.generate_transaction_proof_for_block_body(transaction_index as usize);
    let receipt_mpt_proof = receipts.generate_proof(transaction_index as usize);
    let receipts_root_ssz_proof = block.generate_receipts_root_proof_for_block_body();
    let beacon_header = block.original().block_header();
    let proof = core::TransactionProof {
        header: packed::Header::from_ssz_header(&beacon_header).unpack(),
        receipts_root: receipts.root(),
        transaction_index,
        header_mmr_proof: header_mmr_proof.to_owned(),
        transaction_ssz_proof,
        receipt_mpt_proof,
        receipts_root_ssz_proof,
    };
    Ok(proof.pack())
}

pub fn generate_packed_payload(
    block: &CachedBeaconBlock<MainnetEthSpec>,
    tx: &Transaction,
    receipts: &Receipts,
    transaction_index: u64,
) -> Result<packed::TransactionPayload> {
    let beacon_tx = block
        .transaction(transaction_index as usize)
        .expect("block transaction")
        .to_vec();
    if beacon_tx != tx.rlp().to_vec() {
        return Err(eyre::eyre!("execution and beacon tx is different"));
    }
    let payload = core::TransactionPayload {
        transaction: beacon_tx,
        receipt: receipts.encode_data(transaction_index as usize),
    };
    Ok(payload.pack())
}

pub fn assemble_partial_verification_transaction(
    packed_proof: &packed::TransactionProof,
    packed_payload: &packed::TransactionPayload,
    celldeps: &[CellDep],
) -> Result<TransactionView> {
    let witness = {
        let input_type_args = BytesOpt::new_builder()
            .set(Some(packed_proof.as_slice().pack()))
            .build();
        let output_type_args = BytesOpt::new_builder()
            .set(Some(packed_payload.as_slice().pack()))
            .build();
        let witness_args = WitnessArgs::new_builder()
            .input_type(input_type_args)
            .output_type(output_type_args)
            .build();
        witness_args.as_bytes()
    };
    let tx = TransactionView::new_advanced_builder()
        .cell_deps(celldeps.to_owned())
        .witness(witness.pack())
        .build();
    Ok(tx)
}
