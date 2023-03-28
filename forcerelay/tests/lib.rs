use ckb_jsonrpc_types::Transaction;
use ckb_sdk::traits::SecpCkbRawKeySigner;
use ckb_sdk::unlock::{ScriptSigner, SecpSighashScriptSigner};
use ckb_sdk::{Address, AddressPayload, CkbRpcClient, NetworkType, ScriptGroup, ScriptGroupType};
use ckb_types::core::DepType;
use ckb_types::h256;
use ckb_types::packed::{CellDep, CellInput, CellOutput, OutPoint, Script, ScriptOpt};
use ckb_types::{core::ScriptHashType, prelude::*};
use jsonrpc_core::{self, Output, Response};

use client::{Client, ClientBuilder};
use config::networks;
use eyre::Result;
use secp256k1::{PublicKey, Secp256k1, SecretKey};

use std::process::{Command, Stdio};
use std::str::FromStr;
use std::thread;
use std::time;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

#[ignore]
#[tokio::test]
async fn integration_test() {
    let mmr_root_storage = "ckb_mmr_storage";
    let _ = std::fs::remove_dir_all(mmr_root_storage);

    let mut ckb_working_dir = std::env::current_dir().unwrap();

    let ckb_path = "tests/ckb-dev";
    ckb_working_dir.push(ckb_path);

    let _ = std::fs::remove_dir_all(ckb_path);
    std::fs::create_dir(ckb_path).unwrap();

    let (mut ckb_run, mut ckb_miner) = {
        Command::new("ckb")
            .arg("init")
            .arg("--chain")
            .arg("dev")
            .current_dir(&ckb_working_dir)
            .spawn()
            .unwrap();

        thread::sleep(Duration::from_secs(5));

        std::fs::copy("tests/ckb/ckb.toml", format!("{}/ckb.toml", ckb_path)).unwrap();
        std::fs::copy(
            "tests/ckb/ckb-miner.toml",
            format!("{}/ckb-miner.toml", ckb_path),
        )
        .unwrap();
        std::fs::copy("tests/ckb/dev.toml", format!("{}/specs/dev.toml", ckb_path)).unwrap();

        let ckb_run = Command::new("ckb")
            .arg("run")
            .current_dir(&ckb_working_dir)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();

        thread::sleep(Duration::from_secs(1));

        let ckb_miner = Command::new("ckb")
            .arg("miner")
            .current_dir(&ckb_working_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        thread::sleep(Duration::from_secs(10));

        let tx_rpc_file1 = "tests/ckb/deploy-verify-lightclient.json";

        // deploy the contracts
        // In this contract, we deploy:
        // - lightclient cell (used when updating the mmr root cell, we do not use this cell though)
        // - verify cell
        // - mock business contract. (It will check whether an ETH tx is committed in ETH)
        Command::new("curl")
            .arg("-H")
            .arg("content-type: application/json")
            .arg("-d")
            .arg(format!("@{}", tx_rpc_file1))
            .arg("http://localhost:8114")
            .spawn()
            .unwrap();

        let wait_for_tx_commit = time::Duration::from_secs(5);
        thread::sleep(wait_for_tx_commit);

        let tx_rpc_file2 = "tests/ckb/deploy-mmr-root.json";

        // deploy the mmr root
        // In this contract, we deploy a mmr root cell whose:
        // min slot is 5787808
        // max slot is 5787839
        // Here the type script of this cell is an empty lock to avoid the check of lightclient cell.
        Command::new("curl")
            .arg("-H")
            .arg("content-type: application/json")
            .arg("-d")
            .arg(format!("@{}", tx_rpc_file2))
            .arg("http://localhost:8114")
            .spawn()
            .unwrap();

        let wait_for_tx_commit = time::Duration::from_secs(5);
        thread::sleep(wait_for_tx_commit);

        (ckb_run, ckb_miner)
    };

    let mut eth_server = Command::new("python3")
        .arg("tests/eth_mock/server.py")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();

    let five_secs = time::Duration::from_secs(5);
    thread::sleep(five_secs);

    std::thread::spawn(move || {
        let (client, _sender) = get_client();
        run_verifier(client)
    });

    let wait_for_verifier_bootstrap = time::Duration::from_secs(35);
    thread::sleep(wait_for_verifier_bootstrap);

    // fetch partial tx
    let data = r#"{"jsonrpc":"2.0","method":"forcerelay_getForcerelayCkbTransaction","params":["0x40c6bbef3f8cb9681e0bafc42d20ec421c706633324496b7434ac719e824e027"],"id":1}"#;
    let output = Command::new("curl")
        .arg("-H")
        .arg("content-type: application/json")
        .arg("-X")
        .arg("POST")
        .arg("--no-progress-meter")
        .arg("http://127.0.0.1:8545")
        .arg("--data")
        .arg(data)
        .output()
        .unwrap()
        .stdout;

    // let partial_tx = TransactionView::from_slice(&output)
    let resp = Response::from_json(std::str::from_utf8(&output).unwrap()).unwrap();
    let partial_tx = match resp {
        Response::Single(value) => match value {
            Output::Success(s) => {
                let r = s.result.to_string();
                serde_json::from_str::<Transaction>(&r).unwrap()
            }
            Output::Failure(f) => panic!("failure: {:?}", f),
        },
        Response::Batch(_) => panic!("unexpected response: {:?}", resp),
    };

    let five_secs = time::Duration::from_secs(5);
    thread::sleep(five_secs);

    let tx = finish_tx(partial_tx);
    std::thread::spawn(move || {
        let mut rpc_client = CkbRpcClient::new("http://localhost:8114");
        let _ = rpc_client.send_transaction(tx, None).unwrap();
    });

    let five_secs = time::Duration::from_secs(5);
    thread::sleep(five_secs);

    let _ = ckb_miner.kill();
    let _ = ckb_run.kill();
    let _ = eth_server.kill();
}

// We add an output whose lock script is secp256k1 and type script points to the business contract.
fn finish_tx(partial_tx: Transaction) -> Transaction {
    let packed_tx = ckb_types::packed::Transaction::from(partial_tx);

    let input = CellInput::new_builder()
        .previous_output(
            OutPoint::new_builder()
                .tx_hash(
                    h256!("0x05536ddb4f8c5087d1b3ff9d708f375773790a49cb440baeaa87940b36e434de")
                        .pack(),
                )
                .index(1u32.pack())
                .build(),
        )
        .build();

    let secret_key =
        SecretKey::from_str("63d86723e08f0f813a36ce6aa123bb2289d90680ae1e99d4de8cdb334553f24d")
            .unwrap();

    let lock_script = {
        let public_key = PublicKey::from_secret_key(&Secp256k1::signing_only(), &secret_key);
        let address_payload = AddressPayload::from_pubkey(&public_key);
        let addr = Address::new(NetworkType::Dev, address_payload, true);
        Script::from(&addr)
    };

    let output = {
        let mmr_root_type_script_hash =
            h256!("0x82e841e0d3b248bd9c000ccf2594988ee17d951286a6ddc37065f8087bb09c15").pack();
        let verify_bin_script_hash =
            h256!("0x9ea73e5003f580eb4f380944b1de0711c6b5a4bb96c6f9bf8186203b7c684606").pack();

        let mut args = Vec::<u8>::with_capacity(64);
        args.extend_from_slice(mmr_root_type_script_hash.raw_data().as_ref());
        args.extend_from_slice(verify_bin_script_hash.raw_data().as_ref());
        let type_script = Script::new_builder()
            .code_hash(
                h256!("0xdbdd281ecfaeb662a6983e7ac83fc503a5f20ad6a089767ebd2c70aafc25459a").pack(),
            )
            .hash_type(ScriptHashType::Type.into())
            .args(args.pack())
            .build();
        CellOutput::new_builder()
            .capacity(20_000_000_000_000_u64.pack())
            .lock(lock_script.clone())
            .type_(ScriptOpt::new_builder().set(Some(type_script)).build())
            .build()
    };

    let tx = packed_tx
        .as_advanced_builder()
        .cell_dep(CellDep::new_builder()
            .dep_type(DepType::DepGroup.into())
            .out_point(OutPoint::new_builder()
                .tx_hash(
                    h256!("0x29ed5663501cd171513155f8939ad2c9ffeb92aa4879d39cde987f8eb6274407")
                        .pack(),
                )
                .build())
            .build())
        .cell_dep(CellDep::new_builder()
            .dep_type(DepType::Code.into())
            .out_point(OutPoint::new_builder()
                .tx_hash(
                    h256!("0xb89044d4470d20e1f8b5e981ed0bb4ea7b21bc4f7feebd0c002bf7c8585ce9da").pack()
                )
                .index(3u32.pack())
                .build())
            .build())
        .input(input)
        .output(output)
        .output_data(vec![0u8].pack())
        .build();

    let signer =
        SecpSighashScriptSigner::new(Box::new(SecpCkbRawKeySigner::new_with_secret_keys(vec![
            secret_key,
        ])));

    let tx = signer
        .sign_tx(
            &tx,
            &ScriptGroup {
                script: lock_script,
                group_type: ScriptGroupType::Lock,
                input_indices: vec![0],
                output_indices: vec![], // why should i assign the output in signing inputs?
            },
        )
        .unwrap();

    println!("business tx hash: {:?}", tx.hash());
    tx.data().into()
}

fn get_client() -> (Client, Sender<()>) {
    let checkpoint = "0xe06056afdb9a0a9fd7fbaf89bb0e96eced24de0104bc5b7e3960c115d6990f90";

    let (client, sender) = ClientBuilder::new()
        .network(networks::Network::MAINNET)
        .consensus_rpc("http://127.0.0.1:8444")
        .execution_rpc("http://127.0.0.1:8444")
        .rpc_port(8545)
        .ckb_rpc("http://127.0.0.1:8114")
        .checkpoint(checkpoint)
        .storage_path("ckb_mmr_storage".into())
        .lightclient_contract_typeargs(
            "0xf49ce32397c6741998b04d7548c5ed372007424daf67ee5bfadaefec3c865781",
        )
        .lightclient_binary_typeargs(
            "0xfbe09e8ff3e5f3d0fab7cc7431feed2131846184d356a9626639f55e7f471846",
        )
        .ibc_client_id("ibc-ckb-1")
        .build()
        .unwrap();
    (client, sender)
}

#[tokio::main]
async fn run_verifier(mut client: Client) -> Result<()> {
    client.start().await?;

    std::future::pending().await
}
