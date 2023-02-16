use std::{path::PathBuf, sync::Arc};
use tempfile::TempDir;

use config::{networks, Config};
use consensus::{rpc::mock_rpc::MockRpc, ConsensusClient};

async fn setup(path: PathBuf) -> ConsensusClient<MockRpc> {
    let base_config = networks::goerli();
    let config = Config {
        consensus_rpc: String::new(),
        execution_rpc: String::new(),
        chain: base_config.chain,
        forks: base_config.forks,
        max_checkpoint_age: 123123123,
        storage_path: path,
        ..Default::default()
    };

    let checkpoint =
        hex::decode("1e591af1e90f2db918b2a132991c7c2ee9a4ab26da496bd6e71e4f0bd65ea870").unwrap();

    ConsensusClient::new("testdata/", &checkpoint, Arc::new(config)).unwrap()
}

#[tokio::test]
async fn test_sync() {
    let storage = TempDir::new().unwrap();
    let mut client = setup(storage.into_path()).await;
    client.sync(3781056).await.expect("sync");

    let head = client.get_header();
    assert_eq!(head.slot, 3790918);

    let finalized_head = client.get_finalized_header();
    assert_eq!(finalized_head.slot, 3790848);
}

#[tokio::test]
#[should_panic = "payload: invalid header hash found: 0x1f80…1b7f, expected: 0x75b0d40fd8fb98e5535ee63c242bf2fbeb36a00ca59729cb5ae9f4b7d89522dc"]
async fn test_get_payload() {
    let storage = TempDir::new().unwrap();
    let mut client = setup(storage.into_path()).await;
    client.sync(3781056).await.expect("sync");

    let payload = client
        .get_execution_payload(&None, true)
        .await
        .expect("payload")
        .unwrap();
    assert_eq!(payload.block_number, 7530932);
}

#[tokio::test]
#[ignore]
async fn fetch_headers_into_testdata() {
    use consensus::rpc::{mock_rpc::MockRpc, nimbus_rpc::NimbusRpc, ConsensusRpc};
    use std::collections::BTreeMap;

    const MOCK_RPC: &str = "testdata/";
    const EXPORT_PATH: &str = "testdata";
    const NIMBUS_RPC: &str = "https://www.lightclientdata.org";
    const STEP: u64 = 128;

    let mock_rpc = MockRpc::new(MOCK_RPC);
    let updates = mock_rpc.get_updates(461, 2).await.expect("get_udpates");
    let mut headers = BTreeMap::new();
    if let (Some(start), Some(end)) = (updates.first(), updates.last()) {
        let rpc = NimbusRpc::new(NIMBUS_RPC);
        let mut start_slot = start.finalized_header.slot;
        let target_slot = end.finalized_header.slot;
        println!("fetch slot [{start_slot}, {target_slot}], step = {STEP}");
        while start_slot < target_slot {
            let mut end_slot = start_slot + STEP;
            if end_slot >= target_slot {
                end_slot = target_slot + 1;
            }
            println!("fetch batch slot [{start_slot}, {end_slot})");
            let futrues = (start_slot..end_slot)
                .map(|slot| (slot, rpc.get_header(slot)))
                .collect::<Vec<_>>();
            for (slot, future) in futrues {
                if let Ok(Some(header)) = future.await {
                    headers.insert(slot, header);
                } else {
                    headers.insert(
                        slot,
                        types::Header {
                            slot,
                            ..Default::default()
                        },
                    );
                }
            }
            start_slot += STEP;
        }
    }
    let contents = serde_json::to_string_pretty(&headers).expect("jsonify");
    std::fs::write(format!("{EXPORT_PATH}/headers.json"), contents).expect("write fs");
}
