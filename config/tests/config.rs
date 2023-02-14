use config::Config;
use std::path::Path;

#[test]
fn test_load_full_config() {
    let path = Path::new("./helios.toml");

    let config = Config::from_file(&path.to_path_buf(), "mainnet", &Default::default());
    assert_eq!(config.ckb_rpc, "https://testnet.ckbapp.dev");
}
