use std::cmp;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use blst::min_pk::PublicKey;
use chrono::Duration;
use eth2_types::MainnetEthSpec;
use eth_light_client_in_ckb_verification::{mmr::ClientRootMMR, types::core};
use eyre::{eyre, Result};
use log::{debug, info, warn};
use ssz_rs::prelude::*;
use storage::{
    prelude::{StorageAsMMRStore as _, StorageReader as _, StorageWriter as _},
    Storage,
};
use tree_hash::TreeHash;

use common::types::*;
use common::utils::*;
use config::Config;

use crate::constants::{MAX_REQUEST_LIGHT_CLIENT_UPDATES, MAX_REQUEST_RPC_UPDATES, MAX_RPC_RETRY};
use crate::errors::ConsensusError;

use super::rpc::ConsensusRpc;
use super::types::*;
use super::utils::*;

// https://github.com/ethereum/consensus-specs/blob/dev/specs/altair/light-client/sync-protocol.md
// does not implement force updates

pub struct ConsensusClient<R: ConsensusRpc> {
    pub rpc: R,
    store: LightClientStore,
    initial_checkpoint: Vec<u8>,
    pub last_checkpoint: Option<Vec<u8>>,
    pub config: Arc<Config>,
}

struct LightClientStore {
    base_slot: u64,
    finalized_header: Header,
    current_sync_committee: SyncCommittee,
    next_sync_committee: Option<SyncCommittee>,
    next_sync_committee_branch: Option<Vec<Bytes32>>,
    optimistic_header: Header,
    previous_max_active_participants: u64,
    current_max_active_participants: u64,
    storage: Storage<MainnetEthSpec>,
}

impl<R: ConsensusRpc> ConsensusClient<R> {
    pub fn new(
        rpc: &str,
        checkpoint_block_root: &[u8],
        config: Arc<Config>,
    ) -> Result<ConsensusClient<R>> {
        let rpc = R::new(rpc);

        let storage_path = &config.storage_path;
        let storage = Storage::new(storage_path)?;

        let store = LightClientStore {
            base_slot: 0,
            finalized_header: Default::default(),
            current_sync_committee: Default::default(),
            next_sync_committee: None,
            next_sync_committee_branch: None,
            optimistic_header: Default::default(),
            previous_max_active_participants: 0,
            current_max_active_participants: 0,
            storage,
        };

        Ok(ConsensusClient {
            rpc,
            store,
            last_checkpoint: None,
            config,
            initial_checkpoint: checkpoint_block_root.to_vec(),
        })
    }

    pub fn storage(&self) -> &Storage<MainnetEthSpec> {
        &self.store.storage
    }

    pub fn storage_slot_range(&self) -> Result<(Option<u64>, Option<u64>)> {
        let base_slot = self.storage().get_base_beacon_header_slot()?;
        let tip_slot = self.storage().get_tip_beacon_header_slot()?;
        Ok((base_slot, tip_slot))
    }

    pub async fn get_execution_payload(
        &self,
        slot: &Option<u64>,
        strict: bool,
    ) -> Result<Option<ExecutionPayload>> {
        let slot = slot.unwrap_or(self.store.optimistic_header.slot);
        let block = if let Some(block) = self.rpc.get_block_ssz(slot).await? {
            block
        } else {
            return Ok(None);
        };
        if strict {
            let block_hash = block.tree_hash_root();

            let latest_slot = self.store.optimistic_header.slot;
            let finalized_slot = self.store.finalized_header.slot;

            let verified_block_hash = if slot == latest_slot {
                self.store.optimistic_header.clone().hash_tree_root()?
            } else if slot == finalized_slot {
                self.store.finalized_header.clone().hash_tree_root()?
            } else {
                return Err(ConsensusError::PayloadNotFound(slot).into());
            };

            if verified_block_hash.as_bytes() != block_hash.as_bytes() {
                return Err(ConsensusError::InvalidHeaderHash(
                    block_hash.to_string(),
                    verified_block_hash.to_string(),
                )
                .into());
            }
        }
        let payload = match block.body().execution_payload() {
            Ok(payload) => payload.execution_payload_ref().clone_from_ref(),
            Err(err) => return Err(eyre!(format!("invalid execution_payload: {err:?}"))),
        };
        Ok(Some(payload))
    }

    pub fn get_header(&self) -> &Header {
        &self.store.optimistic_header
    }

    pub fn get_finalized_header(&self) -> &Header {
        &self.store.finalized_header
    }

    fn store_finalized_update_batch(&self, updates: &[Update]) -> Result<()> {
        let storage = self.storage();
        let mut stored_tip_slot = match storage.get_tip_beacon_header_slot()? {
            Some(slot) => slot,
            None => {
                if storage.is_initialized()? {
                    return Err(eyre!("empty stored tip header slot"));
                }
                0
            }
        };
        let base_slot = self.store.base_slot;
        let mut storage_mmr: Option<ClientRootMMR<_>> = None;
        for update in updates {
            let header = &update.finalized_header;
            let header_with_cache = {
                let header: core::Header = header.into();
                header.calc_cache()
            };
            let slot = header.slot;
            let digest = header_with_cache.digest();
            match slot.cmp(&base_slot) {
                cmp::Ordering::Greater => {
                    if stored_tip_slot + 1 == slot {
                        if let Some(mmr) = storage_mmr.as_mut() {
                            mmr.push(digest)?;
                        } else {
                            let mut mmr = storage.chain_root_mmr(stored_tip_slot)?;
                            mmr.push(digest)?;
                            storage_mmr = Some(mmr);
                        }
                        stored_tip_slot = slot;
                    } else {
                        return Err(eyre!(
                            "tip slot should be checked: {slot} != {stored_tip_slot} + 1"
                        ));
                    }
                }
                cmp::Ordering::Equal => {
                    if !storage.is_initialized()? {
                        storage.initialize_with(slot, digest)?;
                        stored_tip_slot = slot;
                    } else {
                        return Err(eyre!("storage should be checked"));
                    }
                }
                cmp::Ordering::Less => {
                    return Err(eyre!("base slot should be checked: {slot} < {base_slot}"));
                }
            }
        }
        storage.put_tip_beacon_header_slot(stored_tip_slot)?;
        if let Some(mmr) = storage_mmr {
            mmr.commit()?;
        }
        Ok(())
    }

    async fn store_updates_from_rpc(&mut self, start_slot: u64, end_slot: u64) -> Result<()> {
        let tasks: Vec<_> = (start_slot..=end_slot)
            .map(|slot| self.get_finality_update(slot))
            .collect();
        let updates: Result<Vec<Update>> =
            futures::future::join_all(tasks).await.into_iter().collect();
        let updates = updates?;
        self.store_finalized_update_batch(&updates)?;
        Ok(())
    }

    pub async fn store_updates_until_finality_update(
        &mut self,
        finality_update: &FinalityUpdate,
    ) -> Result<()> {
        let end_slot = finality_update.finalized_header.slot;
        if let Some(stored_tip_slot) = self.storage().get_tip_beacon_header_slot()? {
            if stored_tip_slot >= end_slot {
                return Ok(());
            }
            debug!("store finalized update for slots ({stored_tip_slot}, {end_slot}]");
            let mut slot_pointer = stored_tip_slot + 1;
            while slot_pointer + MAX_REQUEST_RPC_UPDATES < end_slot {
                self.store_updates_from_rpc(slot_pointer, slot_pointer + MAX_REQUEST_RPC_UPDATES)
                    .await?;
                slot_pointer += MAX_REQUEST_RPC_UPDATES + 1;
            }
            self.store_updates_from_rpc(slot_pointer, end_slot).await
        } else {
            Err(eyre!("tip beacon header slot shouldn't be none"))
        }
    }

    pub async fn get_finality_update(&self, finality_update_slot: u64) -> Result<Update> {
        let mut retry = 0;
        let finalized_header = loop {
            let finalized_header = self.rpc.get_header(finality_update_slot).await;
            if finalized_header.is_ok() || retry < MAX_RPC_RETRY {
                break finalized_header;
            }
            retry += 1;
        };
        let finalized_header = match finalized_header {
            Ok(Some(header)) => header,
            Ok(None) => {
                warn!("forked or skipped beacon header ({finality_update_slot})");
                Header {
                    slot: finality_update_slot,
                    ..Default::default()
                }
            }
            Err(error) => return Err(eyre!("{error}")),
        };
        let update = Update::from_finalized_header(finalized_header);
        Ok(update)
    }

    pub async fn sync(&mut self, base_slot: u64) -> Result<()> {
        info!(
            "consensus client sync with checkpoint: 0x{}",
            hex::encode(&self.initial_checkpoint)
        );
        self.bootstrap(base_slot).await?;

        let current_period = calc_sync_period(self.store.finalized_header.slot);
        let updates = self
            .rpc
            .get_updates(current_period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await?;

        for update in updates {
            self.verify_update(&update)?;
            self.apply_update(&update);
            if update.finalized_header.slot > base_slot {
                self.store_updates_until_finality_update(&update.into())
                    .await?;
            }
        }

        let finality_update = self.rpc.get_finality_update().await?;
        self.verify_finality_update(&finality_update)?;
        self.apply_finality_update(&finality_update);
        self.store_updates_until_finality_update(&finality_update)
            .await?;

        let optimistic_update = self.rpc.get_optimistic_update().await?;
        self.verify_optimistic_update(&optimistic_update)?;
        self.apply_optimistic_update(&optimistic_update);

        info!(
            "consensus client has already synced with slots [{base_slot}, {}]",
            self.get_finalized_header().slot
        );
        Ok(())
    }

    pub async fn advance(&mut self) -> Result<bool> {
        let previous_finality_slot = self.get_finalized_header().slot;
        let finality_update = self.rpc.get_finality_update().await?;
        self.verify_finality_update(&finality_update)?;
        self.apply_finality_update(&finality_update);

        let optimistic_update = self.rpc.get_optimistic_update().await?;
        self.verify_optimistic_update(&optimistic_update)?;
        self.apply_optimistic_update(&optimistic_update);

        if self.store.next_sync_committee.is_none() {
            debug!("checking for sync committee update");
            let current_period = calc_sync_period(self.store.finalized_header.slot);
            let mut updates = self.rpc.get_updates(current_period, 1).await?;

            if updates.len() == 1 {
                let update = updates.get_mut(0).unwrap();
                let res = self.verify_update(update);

                if res.is_ok() {
                    info!("updating sync committee");
                    self.apply_update(update);
                }
            }
        }
        self.store_updates_until_finality_update(&finality_update)
            .await?;

        Ok(previous_finality_slot < self.get_finalized_header().slot)
    }

    pub async fn bootstrap(&mut self, base_slot: u64) -> Result<()> {
        if let Some(stored_base_slot) = self.storage().get_base_beacon_header_slot()? {
            if stored_base_slot != base_slot {
                panic!(
                    "The minimal slot in the light client cell on CKB chain \
                    is not the same as the cached minimal slot in the storage.\n\
                    Please check if the configured light client cell is correct! \n\
                    If you have changed the light client cell, you should delete the local storage."
                );
            }
        }

        let mut bootstrap = self
            .rpc
            .get_bootstrap(&self.initial_checkpoint)
            .await
            .map_err(|_| eyre!("could not fetch bootstrap"))?;

        if bootstrap.header.slot > base_slot {
            return Err(ConsensusError::CheckpointTooNew.into());
        }

        let is_valid = self.is_valid_checkpoint(bootstrap.header.slot);

        if !is_valid {
            if self.config.strict_checkpoint_age {
                return Err(ConsensusError::CheckpointTooOld.into());
            } else {
                warn!("checkpoint too old, consider using a more recent block");
            }
        }

        let committee_valid = is_current_committee_proof_valid(
            &bootstrap.header,
            &mut bootstrap.current_sync_committee,
            &bootstrap.current_sync_committee_branch,
        );

        let header_hash = bootstrap.header.hash_tree_root()?.to_string();
        let expected_hash = format!("0x{}", hex::encode(&self.initial_checkpoint));
        let header_valid = header_hash == expected_hash;

        if !header_valid {
            return Err(ConsensusError::InvalidHeaderHash(expected_hash, header_hash).into());
        }

        if !committee_valid {
            return Err(ConsensusError::InvalidCurrentSyncCommitteeProof.into());
        }

        self.store.base_slot = base_slot;
        self.store.finalized_header = bootstrap.header.clone();
        self.store.current_sync_committee = bootstrap.current_sync_committee;
        self.store.optimistic_header = bootstrap.header;

        // force to initialize mmr storage with on-chain base slot
        if !self.storage().is_initialized()? {
            debug!("initialize mmr stroage in bootstrap for base slot {base_slot}");
            let header = self
                .rpc
                .get_header(base_slot)
                .await?
                .expect("forked or skipped base slot");
            let update = Update::from_finalized_header(header);
            self.store_finalized_update_batch(&[update])?;
        }

        Ok(())
    }

    // implements checks from validate_light_client_update and process_light_client_update in the
    // specification
    fn verify_generic_update(&self, update: &mut GenericUpdate) -> Result<()> {
        let bits = get_bits(&update.sync_aggregate.sync_committee_bits);
        if bits == 0 {
            return Err(ConsensusError::InsufficientParticipation.into());
        }

        let update_finalized_slot = update
            .finalized_header
            .as_ref()
            .map(|header| header.slot)
            .unwrap_or(0);
        let valid_time = update.signature_slot > update.attested_header.slot
            && update.attested_header.slot >= update_finalized_slot;

        if !valid_time {
            return Err(ConsensusError::InvalidTimestamp.into());
        }

        let store_period = calc_sync_period(self.store.finalized_header.slot);
        let update_sig_period = calc_sync_period(update.signature_slot);
        let valid_period = if self.store.next_sync_committee.is_some() {
            update_sig_period == store_period || update_sig_period == store_period + 1
        } else {
            update_sig_period == store_period
        };

        if !valid_period {
            return Err(ConsensusError::InvalidPeriod.into());
        }

        let update_attested_period = calc_sync_period(update.attested_header.slot);
        let update_has_next_committee = self.store.next_sync_committee.is_none()
            && update.next_sync_committee.is_some()
            && update_attested_period == store_period;

        if update.attested_header.slot <= self.store.finalized_header.slot
            && !update_has_next_committee
        {
            return Err(ConsensusError::NotRelevant.into());
        }

        if update.finalized_header.is_some() && update.finality_branch.is_some() {
            let is_valid = is_finality_proof_valid(
                &update.attested_header,
                update.finalized_header.as_mut().unwrap(),
                update.finality_branch.as_ref().unwrap(),
            );

            if !is_valid {
                return Err(ConsensusError::InvalidFinalityProof.into());
            }
        }

        if update.next_sync_committee.is_some() && update.next_sync_committee_branch.is_some() {
            let is_valid = is_next_committee_proof_valid(
                &update.attested_header,
                update.next_sync_committee.as_mut().unwrap(),
                update.next_sync_committee_branch.as_ref().unwrap(),
            );

            if !is_valid {
                return Err(ConsensusError::InvalidNextSyncCommitteeProof.into());
            }
        }

        let sync_committee = if update_sig_period == store_period {
            &self.store.current_sync_committee
        } else {
            self.store.next_sync_committee.as_ref().unwrap()
        };

        let pks =
            get_participating_keys(sync_committee, &update.sync_aggregate.sync_committee_bits)?;

        let is_valid_sig = self.verify_sync_committee_signture(
            &pks,
            &update.attested_header,
            &update.sync_aggregate.sync_committee_signature,
            update.signature_slot,
        );

        if !is_valid_sig {
            return Err(ConsensusError::InvalidSignature.into());
        }

        Ok(())
    }

    fn verify_update(&self, update: &Update) -> Result<()> {
        let mut update = GenericUpdate::from(update);
        self.verify_generic_update(&mut update)
    }

    fn verify_finality_update(&self, update: &FinalityUpdate) -> Result<()> {
        let mut update = GenericUpdate::from(update);
        self.verify_generic_update(&mut update)
    }

    fn verify_optimistic_update(&self, update: &OptimisticUpdate) -> Result<()> {
        let mut update = GenericUpdate::from(update);
        self.verify_generic_update(&mut update)
    }

    // implements state changes from apply_light_client_update and process_light_client_update in
    // the specification
    fn apply_generic_update(&mut self, update: GenericUpdate) {
        let committee_bits = get_bits(&update.sync_aggregate.sync_committee_bits);

        self.store.current_max_active_participants =
            u64::max(self.store.current_max_active_participants, committee_bits);

        let should_update_optimistic = committee_bits > self.safety_threshold()
            && update.attested_header.slot > self.store.optimistic_header.slot;

        if should_update_optimistic {
            self.store.optimistic_header = update.attested_header.clone();
        }

        let update_attested_period = calc_sync_period(update.attested_header.slot);

        let update_finalized_slot = update
            .finalized_header
            .as_ref()
            .map(|h| h.slot)
            .unwrap_or(0);

        let update_finalized_period = calc_sync_period(update_finalized_slot);

        let update_has_finalized_next_committee = self.store.next_sync_committee.is_none()
            && self.has_sync_update(&update)
            && self.has_finality_update(&update)
            && update_finalized_period == update_attested_period;

        let should_apply_update = {
            let has_majority = committee_bits * 3 >= 512 * 2;
            let update_is_newer = update_finalized_slot > self.store.finalized_header.slot;
            let good_update = update_is_newer || update_has_finalized_next_committee;

            has_majority && good_update
        };

        if should_apply_update {
            let store_period = calc_sync_period(self.store.finalized_header.slot);

            if self.store.next_sync_committee.is_none() {
                self.store.next_sync_committee = update.next_sync_committee;
                self.store.next_sync_committee_branch = update.next_sync_committee_branch;
            } else if update_finalized_period == store_period + 1 {
                self.store.current_sync_committee = self.store.next_sync_committee.clone().unwrap();
                self.store.next_sync_committee = update.next_sync_committee;
                self.store.next_sync_committee_branch = update.next_sync_committee_branch;
                self.store.previous_max_active_participants =
                    self.store.current_max_active_participants;
                self.store.current_max_active_participants = 0;
            }

            if update_finalized_slot > self.store.finalized_header.slot {
                self.store.finalized_header = update.finalized_header.unwrap();

                if self.store.finalized_header.slot % 32 == 0 {
                    let checkpoint_res = self.store.finalized_header.hash_tree_root();
                    if let Ok(checkpoint) = checkpoint_res {
                        self.last_checkpoint = Some(checkpoint.as_bytes().to_vec());
                    }
                }

                if self.store.finalized_header.slot > self.store.optimistic_header.slot {
                    self.store.optimistic_header = self.store.finalized_header.clone();
                }
            }
        }
    }

    fn apply_update(&mut self, update: &Update) {
        let update = GenericUpdate::from(update);
        self.apply_generic_update(update);
    }

    fn apply_finality_update(&mut self, update: &FinalityUpdate) {
        let update = GenericUpdate::from(update);
        self.apply_generic_update(update);
    }

    fn apply_optimistic_update(&mut self, update: &OptimisticUpdate) {
        let update = GenericUpdate::from(update);
        self.apply_generic_update(update);
    }

    fn has_finality_update(&self, update: &GenericUpdate) -> bool {
        update.finalized_header.is_some() && update.finality_branch.is_some()
    }

    fn has_sync_update(&self, update: &GenericUpdate) -> bool {
        update.next_sync_committee.is_some() && update.next_sync_committee_branch.is_some()
    }

    fn safety_threshold(&self) -> u64 {
        cmp::max(
            self.store.current_max_active_participants,
            self.store.previous_max_active_participants,
        ) / 2
    }

    fn verify_sync_committee_signture(
        &self,
        pks: &[PublicKey],
        attested_header: &Header,
        signature: &SignatureBytes,
        signature_slot: u64,
    ) -> bool {
        let res: Result<bool> = (move || {
            let pks: Vec<&PublicKey> = pks.iter().collect();
            let header_root =
                bytes_to_bytes32(attested_header.clone().hash_tree_root()?.as_bytes());
            let signing_root = self.compute_committee_sign_root(header_root, signature_slot)?;

            Ok(is_aggregate_valid(signature, signing_root.as_bytes(), &pks))
        })();

        if let Ok(is_valid) = res {
            is_valid
        } else {
            false
        }
    }

    fn compute_committee_sign_root(&self, header: Bytes32, slot: u64) -> Result<Node> {
        let genesis_root = self.config.chain.genesis_root.to_vec().try_into().unwrap();

        let domain_type = &hex::decode("07000000")?[..];
        let fork_version = Vector::from_iter(self.config.fork_version(slot));
        let domain = compute_domain(domain_type, fork_version, genesis_root)?;
        compute_signing_root(header, domain)
    }

    pub fn expected_current_slot(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap();

        let genesis_time = self.config.chain.genesis_time;
        let since_genesis = now - std::time::Duration::from_secs(genesis_time);

        since_genesis.as_secs() / 12
    }

    fn slot_timestamp(&self, slot: u64) -> u64 {
        slot * 12 + self.config.chain.genesis_time
    }

    /// Gets the duration until the next update
    /// Updates are scheduled for 4 seconds into each slot
    pub fn duration_until_next_update(&self) -> Duration {
        let current_slot = self.expected_current_slot();
        let next_slot = current_slot + 1;
        let next_slot_timestamp = self.slot_timestamp(next_slot);

        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let time_to_next_slot = next_slot_timestamp.saturating_sub(now);
        let next_update = time_to_next_slot + 4;

        Duration::seconds(next_update as i64)
    }

    // Determines blockhash_slot age and returns true if it is less than 14 days old
    fn is_valid_checkpoint(&self, blockhash_slot: u64) -> bool {
        let current_slot = self.expected_current_slot();
        let current_slot_timestamp = self.slot_timestamp(current_slot);
        let blockhash_slot_timestamp = self.slot_timestamp(blockhash_slot);

        let slot_age = current_slot_timestamp.saturating_sub(blockhash_slot_timestamp);
        slot_age < self.config.max_checkpoint_age
    }
}

fn get_participating_keys(
    committee: &SyncCommittee,
    bitfield: &Bitvector<512>,
) -> Result<Vec<PublicKey>> {
    let mut pks: Vec<PublicKey> = Vec::new();
    bitfield.iter().enumerate().for_each(|(i, bit)| {
        if bit == true {
            let pk = &committee.pubkeys[i];
            let pk = PublicKey::from_bytes(pk).unwrap();
            pks.push(pk);
        }
    });

    Ok(pks)
}

fn get_bits(bitfield: &Bitvector<512>) -> u64 {
    let mut count = 0;
    bitfield.iter().for_each(|bit| {
        if bit == true {
            count += 1;
        }
    });

    count
}

fn is_finality_proof_valid(
    attested_header: &Header,
    finality_header: &mut Header,
    finality_branch: &[Bytes32],
) -> bool {
    is_proof_valid(attested_header, finality_header, finality_branch, 6, 41)
}

fn is_next_committee_proof_valid(
    attested_header: &Header,
    next_committee: &mut SyncCommittee,
    next_committee_branch: &[Bytes32],
) -> bool {
    is_proof_valid(
        attested_header,
        next_committee,
        next_committee_branch,
        5,
        23,
    )
}

fn is_current_committee_proof_valid(
    attested_header: &Header,
    current_committee: &mut SyncCommittee,
    current_committee_branch: &[Bytes32],
) -> bool {
    is_proof_valid(
        attested_header,
        current_committee,
        current_committee_branch,
        5,
        22,
    )
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};
    use tempfile::TempDir;

    use crate::constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES;
    use ssz_rs::Vector;

    use crate::{
        consensus::calc_sync_period,
        errors::ConsensusError,
        rpc::{mock_rpc::MockRpc, ConsensusRpc},
        types::Header,
        ConsensusClient,
    };
    use config::{networks, Config};

    async fn get_client(strict_checkpoint_age: bool, path: PathBuf) -> ConsensusClient<MockRpc> {
        let base_config = networks::goerli();
        let config = Config {
            consensus_rpc: String::new(),
            execution_rpc: String::new(),
            chain: base_config.chain,
            forks: base_config.forks,
            storage_path: path,
            strict_checkpoint_age,
            ..Default::default()
        };

        let checkpoint =
            hex::decode("1e591af1e90f2db918b2a132991c7c2ee9a4ab26da496bd6e71e4f0bd65ea870")
                .unwrap();

        let mut client = ConsensusClient::new("testdata/", &checkpoint, Arc::new(config)).unwrap();
        client.bootstrap(3781056).await.unwrap();
        client
    }

    #[tokio::test]
    async fn test_verify_update() {
        let storage = TempDir::new().unwrap();
        let client = get_client(false, storage.into_path()).await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let update = updates[0].clone();
        client.verify_update(&update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_update_invalid_committee() {
        let storage = TempDir::new().unwrap();
        let client = get_client(false, storage.into_path()).await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.next_sync_committee.pubkeys[0] = Vector::default();

        let err = client.verify_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidNextSyncCommitteeProof.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_update_invalid_finality() {
        let storage = TempDir::new().unwrap();
        let client = get_client(false, storage.into_path()).await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.finalized_header = Header::default();

        let err = client.verify_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidFinalityProof.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_update_invalid_sig() {
        let storage = TempDir::new().unwrap();
        let client = get_client(false, storage.into_path()).await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.sync_aggregate.sync_committee_signature = Vector::default();

        let err = client.verify_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidSignature.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_finality() {
        let storage = TempDir::new().unwrap();
        let mut client = get_client(false, storage.into_path()).await;
        client.sync(3781056).await.unwrap();

        let update = client.rpc.get_finality_update().await.unwrap();

        client.verify_finality_update(&update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_finality_invalid_finality() {
        let storage = TempDir::new().unwrap();
        let mut client = get_client(false, storage.into_path()).await;
        client.sync(3781056).await.unwrap();

        let mut update = client.rpc.get_finality_update().await.unwrap();
        update.finalized_header = Header::default();

        let err = client.verify_finality_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidFinalityProof.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_finality_invalid_sig() {
        let storage = TempDir::new().unwrap();
        let mut client = get_client(false, storage.into_path()).await;
        client.sync(3781056).await.unwrap();

        let mut update = client.rpc.get_finality_update().await.unwrap();
        update.sync_aggregate.sync_committee_signature = Vector::default();

        let err = client.verify_finality_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidSignature.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_optimistic() {
        let storage = TempDir::new().unwrap();
        let mut client = get_client(false, storage.into_path()).await;
        client.sync(3781056).await.unwrap();

        let update = client.rpc.get_optimistic_update().await.unwrap();
        client.verify_optimistic_update(&update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_optimistic_invalid_sig() {
        let storage = TempDir::new().unwrap();
        let mut client = get_client(false, storage.into_path()).await;
        client.sync(3781056).await.unwrap();

        let mut update = client.rpc.get_optimistic_update().await.unwrap();
        update.sync_aggregate.sync_committee_signature = Vector::default();

        let err = client.verify_optimistic_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidSignature.to_string()
        );
    }

    #[tokio::test]
    #[should_panic]
    async fn test_verify_checkpoint_age_invalid() {
        let storage = TempDir::new().unwrap();
        get_client(true, storage.into_path()).await;
    }
}
