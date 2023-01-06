use ckb_sdk::rpc::ckb_indexer::SearchKey;
use ckb_sdk::traits::{CellQueryOptions, LiveCell, PrimaryScriptType};
use ckb_types::core::{DepType, TransactionView};
use ckb_types::packed::{CellDep, Script};
use consensus::types::Header;
use eth2_types::{BeaconBlockHeader, Hash256, MainnetEthSpec};
use eth_light_client_in_ckb_prover::{CachedBeaconBlock, Receipts};
use eth_light_client_in_ckb_verification::mmr;
use eth_light_client_in_ckb_verification::types::{core, packed, prelude::*};
use ethers::types::{Transaction, TransactionReceipt};
use eyre::Result;
use tree_hash::TreeHash;

use crate::rpc::RpcClient;

pub async fn search_cell(rpc: &RpcClient, typescript: &Script) -> Result<Option<LiveCell>> {
    let search: SearchKey =
        CellQueryOptions::new(typescript.clone(), PrimaryScriptType::Type).into();
    let result = rpc.fetch_live_cells(search, 1, None).await?;
    Ok(result.objects.first().cloned().map(Into::into))
}

pub async fn search_cell_as_celldep(
    rpc: &RpcClient,
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

pub fn find_receipt_index(receipt: &TransactionReceipt, receipts: &Receipts) -> u64 {
    let mut index = 0;
    receipts
        .original()
        .iter()
        .enumerate()
        .for_each(|(i, value)| {
            if value.block_number == receipt.block_number {
                index = i;
            }
        });
    return index as u64;
}

pub fn verify_transaction_raw_data(
    headers: &[BeaconBlockHeader],
    block: &CachedBeaconBlock<MainnetEthSpec>,
    tx: &Transaction,
    receipt: &TransactionReceipt,
    receipts: &Receipts,
) -> Result<()> {
    let store = mmr::lib::util::MemStore::default();
    let mmr = {
        let mut mmr = mmr::ClientRootMMR::new(0, &store);
        for header in headers {
            let header: core::Header = packed::Header::from_ssz_header(header).unpack();
            mmr.push(header.calc_cache().digest()).unwrap();
        }
        mmr
    };
    let last_header = &headers[headers.len() - 1];
    let client = core::Client {
        minimal_slot: headers[0].slot.into(),
        maximal_slot: last_header.slot.into(),
        tip_header_root: last_header.tree_hash_root(),
        headers_mmr_root: mmr.get_root().unwrap().unpack(),
    };
    let header = {
        let mut header = None;
        headers.iter().for_each(|item| {
            if item.slot == block.slot() {
                header = Some(item);
            }
        });
        if header.is_none() {
            return Err(eyre::eyre!("unexpected block slot"));
        }
        header.unwrap()
    };
    let mmr_position = block.slot() - client.minimal_slot;
    let header_mmr_proof = mmr
        .gen_proof(vec![mmr_position.into()])
        .expect("gen mmr proof")
        .proof_items()
        .into_iter()
        .map(Unpack::unpack)
        .collect();
    let transaction_index = find_receipt_index(receipt, receipts);
    let transaction_ssz_proof =
        block.generate_transaction_proof_for_block_body(transaction_index as usize);
    let receipt_mpt_proof = receipts.generate_proof(transaction_index as usize);
    let receipts_root_ssz_proof = block.generate_receipts_root_proof_for_block_body();
    let proof = core::TransactionProof {
        header: packed::Header::from_ssz_header(header).unpack(),
        receipts_root: receipts.root(),
        transaction_index,
        header_mmr_proof,
        transaction_ssz_proof,
        receipt_mpt_proof,
        receipts_root_ssz_proof,
    };
    client
        .verify_packed_transaction_proof(proof.pack().as_reader())
        .map_err(|_| eyre::eyre!("verify proof error"))?;
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
    proof
        .verify_packed_payload(payload.pack().as_reader())
        .map_err(|_| eyre::eyre!("verify proof payload"))?;
    Ok(())
}

pub fn assemble_partial_verification_transaction(
    headers: &[BeaconBlockHeader],
    block: &CachedBeaconBlock<MainnetEthSpec>,
    tx: &Transaction,
    receipt: &TransactionReceipt,
    receipts: &Receipts,
    contract_celldep: &CellDep,
) -> Result<TransactionView> {
    // write verification transaction here
    todo!()
}
