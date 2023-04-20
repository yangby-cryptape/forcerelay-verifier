use std::panic::PanicInfo;
use std::sync::Arc;
use tokio::sync::Mutex;

use clap::Parser;
use common::utils::hex_str_to_bytes;
use dirs::home_dir;
use env_logger::Builder;
use eyre::Result;

use client::ClientBuilder;
use config::{CliConfig, Config};
use log::{debug, warn, LevelFilter};

#[tokio::main]
async fn main() -> Result<()> {
    Builder::new()
        .filter_module("forcerelay", LevelFilter::Trace)
        .filter_module("consensus", LevelFilter::Trace)
        .filter_module("execution", LevelFilter::Trace)
        .filter_module("forceth", LevelFilter::Trace)
        .init();

    // Panics if [`libc::getrlimit`] or [`libc::setrlimit`] fail.
    if let Some(limit) = fdlimit::raise_fd_limit() {
        debug!("raise the soft open file descriptor resource limit to the hard limit (={limit}).");
    }

    let config = get_config();
    let (client, shutdown_notifier) = ClientBuilder::new().config(config).build()?;
    let client = Arc::new(Mutex::new(client));

    let verifier = client.clone();
    tokio::spawn(async move {
        verifier.lock().await.start().await.expect("verifier start");
    });

    let (ctrlc_sender, mut ctrlc_receiver) = tokio::sync::mpsc::channel(1);
    ctrlc::set_handler(move || {
        ctrlc_sender.try_send(()).expect("ctrlc_sender is droped");
    })
    .expect("could not register shutdown handler");

    let (panic_sender, mut panic_receiver) = tokio::sync::mpsc::channel(1);
    std::panic::set_hook(Box::new(move |info: &PanicInfo<'_>| {
        panic_sender
            .try_send(info.to_string())
            .expect("panic_receiver is droped");
    }));

    tokio::select! {
        _ = ctrlc_receiver.recv() => {
            warn!("<Ctrl-C> is pressed, quit Forcerelay/Eth verifier and shutdown");
            shutdown_notifier.send(()).await.expect("shutdown is dropped");
        }
        Some(panic_info) = panic_receiver.recv() => {
            warn!("child thread paniced: {panic_info}");
            shutdown_notifier.send(()).await.expect("shutdown is dropped");
        }
    }

    client.lock().await.shutdown().await;
    Ok(())
}

fn get_config() -> Config {
    let cli = Cli::parse();
    let config_path = home_dir().unwrap().join(".forceth/config.toml");
    let cli_config = cli.as_cli_config();

    Config::from_file(&config_path, &cli.network, &cli_config)
}

#[derive(Parser)]
struct Cli {
    #[clap(short, long, default_value = "mainnet")]
    network: String,
    #[clap(short = 'p', long, env)]
    rpc_port: Option<u16>,
    #[clap(short = 'w', long, env)]
    checkpoint: Option<String>,
    #[clap(short, long, env)]
    execution_rpc: Option<String>,
    #[clap(short, long, env)]
    consensus_rpc: Option<String>,
    #[clap(short = 'f', long, env)]
    fallback: Option<String>,
    #[clap(short = 'l', long, env)]
    load_external_fallback: bool,
    #[clap(short = 's', long, env)]
    strict_checkpoint_age: bool,
}

impl Cli {
    fn as_cli_config(&self) -> CliConfig {
        let checkpoint = self
            .checkpoint
            .as_ref()
            .map(|value| hex_str_to_bytes(value).expect("invalid checkpoint"));

        CliConfig {
            checkpoint,
            execution_rpc: self.execution_rpc.clone(),
            consensus_rpc: self.consensus_rpc.clone(),
            rpc_port: self.rpc_port,
            fallback: self.fallback.clone(),
            load_external_fallback: self.load_external_fallback,
            strict_checkpoint_age: self.strict_checkpoint_age,
        }
    }
}
