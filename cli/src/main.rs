use std::process::exit;
use std::sync::{Arc, Mutex};

use clap::Parser;
use common::utils::hex_str_to_bytes;
use dirs::home_dir;
use env_logger::Builder;
use eyre::Result;

use client::{Client, ClientBuilder};
use config::{CliConfig, Config};
use futures::executor::block_on;
use log::{debug, info, LevelFilter};

#[tokio::main]
async fn main() -> Result<()> {
    Builder::new()
        .filter_module("forcerelay", LevelFilter::Trace)
        .filter_module("consensus", LevelFilter::Trace)
        .filter_module("execution", LevelFilter::Trace)
        .filter_module("cli", LevelFilter::Trace)
        .init();

    // Panics if [`libc::getrlimit`] or [`libc::setrlimit`] fail.
    if let Some(limit) = fdlimit::raise_fd_limit() {
        debug!("raise the soft open file descriptor resource limit to the hard limit (={limit}).");
    }

    let config = get_config();
    let mut client = ClientBuilder::new().config(config).build()?;

    client.start().await?;

    register_shutdown_handler(client);
    std::future::pending().await
}

fn register_shutdown_handler(client: Client) {
    let client = Arc::new(client);
    let shutdown_counter = Arc::new(Mutex::new(0));

    ctrlc::set_handler(move || {
        let mut counter = shutdown_counter.lock().unwrap();
        *counter += 1;

        let counter_value = *counter;

        if counter_value == 3 {
            info!("forced shutdown");
            exit(0);
        }

        info!(
            "shutting down... press ctrl-c {} more times to force quit",
            3 - counter_value
        );

        if counter_value == 1 {
            let client = client.clone();
            std::thread::spawn(move || {
                block_on(client.shutdown());
                exit(0);
            });
        }
    })
    .expect("could not register shutdown handler");
}

fn get_config() -> Config {
    let cli = Cli::parse();

    let config_path = home_dir().unwrap().join(".helios/helios.toml");

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
