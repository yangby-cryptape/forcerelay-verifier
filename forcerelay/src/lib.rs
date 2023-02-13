#[cfg(test)]
use env_logger::{Builder, Target};
#[cfg(test)]
use log::LevelFilter;

pub mod assembler;
pub mod errors;
pub mod forcerelay;
pub mod rpc;
pub mod util;

#[cfg(test)]
pub(crate) fn setup_test_logger() {
    let _ = Builder::new()
        .filter_module("forcerelay", LevelFilter::Trace)
        .filter_module("consensus", LevelFilter::Trace)
        .filter_module("eth_light_client_in_ckb", LevelFilter::Trace)
        .target(Target::Stdout)
        .is_test(true)
        .try_init();
    println!();
}
