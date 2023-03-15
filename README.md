## Forceth (Forcerelay/Eth Verifier)

Forceth is a standalone verifier built from (Helios)[https://github.com/a16z/helios] to generate partial CKB crosschain (ETH -> CKB) transaction.

Forceth works with an on-chain light client which contains a range of Ethereum beacon chain headers and maintains a standalone MMR native storage. The on-chain light client is presented as a CKB live cell and maintained by the Forcerelay/Eth which relays Ethereum beacon chain headers into CKB.

## Installing

```
cargo install --path cli
```

and then, run `forceth`.

### Configuration Files

All configuration options can be set on a per-network level in `~/.forceth/config.toml`. Here is an example config file:

```toml
[mainnet]
consensus_rpc = "https://www.lightclientdata.org"
execution_rpc = "https://eth-mainnet.g.alchemy.com/v2/XXXXXX"
rpc_port = 8545
checkpoint = "0xa179cbd497b112acb057039601a75e2daafae994aa5f01d6e1a1d6f85e07a8ef"
ckb_rpc = "https://testnet.ckbapp.dev"
lightclient_contract_typeargs = "0xb7fcfa4ad253ddd60481bdd35331a634ff985fdb3b3fab4e1066cf97faf40315"
lightclient_binary_typeargs = "0xeb871adf5fc97fde4b56ee8b545581147dbb8eb6fdce4fd3d51e8c3618505699"
storage_path = "./ckb_mmr_storage"
ckb_ibc_client_id = "ibc-ckb-1"

[goerli]
consensus_rpc = "http://testing.prater.beacon-api.nimbus.team"
execution_rpc = "https://eth-mainnet.g.alchemy.com/v2/XXXXXX"
rpc_port = 8545
checkpoint = "0x6c668d888d2b892bf26c25ee937ed43b48048dedda971ff33ebaaae7b2bd3890"
ckb_rpc = "https://testnet.ckbapp.dev"
lightclient_contract_typeargs = "0xb7fcfa4ad253ddd60481bdd35331a634ff985fdb3b3fab4e1066cf97faf40315"
lightclient_binary_typeargs = "0xeb871adf5fc97fde4b56ee8b545581147dbb8eb6fdce4fd3d51e8c3618505699"
storage_path = "./ckb_mmr_storage"
ckb_ibc_client_id = "ibc-ckb-2"
```

A comprehensive breakdown of config options is available in the [config.md](./config.md) file.

## Disclaimer

_Forceth can only work while the on-chain ethereum light client cell on ckb is live, and if the relayer becomes evil, forceth will be unworkable instead of being evil._
