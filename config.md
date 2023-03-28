# Forceth Configuration

All configuration options can be set on a per-network level in `~/.forceth/config.toml`.

## Comprehensive Example

```toml
[mainnet]
# The consensus rpc to use. This should be a trusted rpc endpoint. Defaults to "https://www.lightclientdata.org".
consensus_rpc = "https://www.lightclientdata.org"
# [REQUIRED] The execution rpc to use. This should be a trusted rpc endpoint.
execution_rpc = "https://eth-mainnet.g.alchemy.com/v2/XXXXX"
# The port to run the JSON-RPC server on. By default, Helios will use port 8545.
rpc_port = 8545
# The ckb rpc to use. This should be a trusted ckb rpc endpoint and it should
# support ckb-indexer.
ckb_rpc = "http://127.0.0.1:8114"
# The path for storage eth headers
storage_path = "./ckb_mmr_storage"
# The id of the light client.
ckb_ibc_client_id = "ibc-ckb-1"
# The type args of the light client contract.
lightclient_contract_typeargs = "0xb7fcfa4ad253ddd60481bdd35331a634ff985fdb3b3fab4e1066cf97faf40315"
# The type args of the light client verify contract.
lightclient_binary_typeargs = "0xeb871adf5fc97fde4b56ee8b545581147dbb8eb6fdce4fd3d51e8c3618505699"
# A trusted checkpoint. It should not be modified after verifier is once launched.
checkpoint = "0x85e6151a246e8fdba36db27a0c7678a575346272fe978c9281e13a8b26cdfa68"

[goerli]
# The consensus rpc to use. This should be a trusted rpc endpoint. Defaults to Nimbus testnet.
consensus_rpc = "http://testing.prater.beacon-api.nimbus.team"
# [REQUIRED] The execution rpc to use. This should be a trusted rpc endpoint.
execution_rpc = "https://eth-goerli.g.alchemy.com/v2/XXXXX"
# The port to run the JSON-RPC server on. By default, Helios will use port 8545.
rpc_port = 8545
# The latest checkpoint. This should be a trusted checkpoint that is no greater than ~2 weeks old.
# If you are unsure what checkpoint to use, you can skip this option and set either `load_external_fallback` or `fallback` values (described below) to fetch a checkpoint. Though this is not recommended and less secure.
checkpoint = "0xb5c375696913865d7c0e166d87bc7c772b6210dc9edf149f4c7ddc6da0dd4495"
```

## Options

All configuration options below are available on a per-network level, where network is specified by a header (eg `[mainnet]` or `[goerli]`). Many of these options can be configured through cli flags as well. See [README.md](./README.md#additional-options) or run `helios --help` for more information.

- `consensus_rpc` - The URL of the consensus RPC endpoint used to fetch the latest beacon chain head and sync status. This must be a consenus node that supports the light client beaconchain api. We recommend using Nimbus for this. If no consensus rpc is supplied, it defaults to `https://www.lightclientdata.org` which is run by `lightclientdata`.

- `execution_rpc` - The URL of the execution RPC endpoint used to fetch the latest execution chain head and sync status. This must be an execution node that supports the light client execution api. We recommend using Geth for this.

- `rpc_port` - The port to run the JSON-RPC server on. By default, Helios will use port 8545.

- `checkpoint` - The latest checkpoint. This should be a trusted checkpoint that is no greater than ~2 weeks old. If you are unsure what checkpoint to use, you can skip this option and set either `load_external_fallback` or `fallback` values (described below) to fetch a checkpoint. Though this is not recommended and less secure.
