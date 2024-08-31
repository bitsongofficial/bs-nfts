# Bitsong NFT Scripts

## Contents
This library contains both Rust & Bash scripts for the BS-NFT repository.


| Name | Language | Version | Description |
|----------|----------|----------|----------|
| [**Testing Suite**](./src/test/mod.rs) | `Rust`   | `tbd`  | Integration test suite for all contracts.  |
| [**Cw-Orch Deployment** ](./src/deploy/mod.rs)  | `Rust`   | `tbd`   | Used for production and simulation environment contract deployment workflows.  |
| **Bitsong Account Framework Deployment**  | `Rust` |`tbd`  | Automation scripts for deployment of smart contract and IBC infrastructure that powers Bitsong Accounts.   |


## Current Orchestrator Suites
| Suite Name | Description |
|----------|----------|
| [`BtsgAccountSuite`](./src/deploy/bundles/account.rs#12)| Account Collection, Marketplace, and Minter. |



## Commands 
| Command | Description |
|----------|----------|
| `cargo test` | Run all test in codebase |
| `sh build_shema.sh` | Build cosmwasm schemas for each contract in codebase |
| `cargo run --bin connect-ibc` | Connect IBC between two chains. |
| `cargo run --bin create-remote-account` | Creates remote bitsong account between 2 chains |
| `cargo run --bin deploy-ibc` | Deploy IBC client & host contracts, register to version control contract. |
| `cargo run --bin deploy-modules` | Deploy default account modules & adapters |
| `cargo run --bin full-deploy -- --network-id <chain-id1>, <chain-id2>` | Deploy & register default account framework contracts. |
| `cargo run --bin ibc-test` | Test ibc connection of bitsong accounts between 2 chains |
| `cargo run --bin create-nft-account -- --nft-addr <contract-addr>, --id <token-id>` | Manual deployment of tokenized account framework |
| `cargo run --bin manual-deploy -- --network-ids <chain-id-1> <chain-id-2> ... ` | Manual deployment of account framework |
<!-- | `cargo run --bin migrate` | Test ibc connection of bitsong accounts between 2 chains | -->
<!-- | `sh simulate.sh` | Simulate bs721-bonding-curve iterations | -->