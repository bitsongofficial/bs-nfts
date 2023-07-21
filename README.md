# BitSong NFTs

## Overview

This repository is a fork of the original [cw-nfts](https://github.com/CosmWasm/cw-nfts) project. The purpose of this fork is to provide additional functionality to the NFT collections created with this codebase.

## Changes

The following changes have been made to the codebase:

* Added a `uri` field to the `bs721` package. This field allows for the addition of a URI to the NFT collection, providing additional information and context for each NFT.

* Added royalties information for each NFT token. This allows creators and owners to define a percentage of the sale price to be paid out to the original creator or another designated recipient.

* Added an orchestrator contract `launchparty-fixed` to instantiate and manage NFTs and royalties.

## Codebase

All other parts of the code remain unchanged and identical to the original cw721 project. This fork is intended to provide additional functionality for those looking to build their own NFT collections, without modifying the underlying codebase.

## Getting Started

These instructions will help you get a copy of the smart contracts up and running on your local machine for development and testing purposes.

### Prerequisites

* [CosmWasm](https://github.com/CosmWasm/cosmwasm)
* Rust: [Installation Guide](https://www.rust-lang.org/tools/install)
* Command runner: [cargo-make](https://github.com/sagiegurari/cargo-make)
* [Docker](https://www.docker.com/)

### Installation

1. Clone the repository:

    ```shell
    git clone https://github.com/bitsongofficial/bs-nfts.git
    ```

2. Change into the project directory:

    ```shell
    cd bs-nfts
    ```

3. Launch Docker and then build the smart contract:

    ```shell
    cargo make optimize
    ```

    If you prefer to not install Docker, you can obtain a non-optimized build by running:

    ```shell
    cargo make build
    ```

### Test

To run all workspace tests:

```shell
cargo make test
```

or, if you prefer to run contract-specific tests:

```shell
cargo test -p <PACKAGE_NAME>
```

### Lint

```shell
cargo make lint && cargo make fmt 
```

### JSON Schema

```shell
cargo make schema
```

## Development

## Contributing

If you are working on an NFT project as well and wish to give input, please raise issues and/or PRs.
Additional maintainers can be added if they show commitment to the project.

You can also join the [BitSong Discord](https://discord.bitsong.io) server
for more interactive discussions on these themes.
