# Bs721 Basic

This is a basic implementation of a bs721 NFT contract. It implements
the [BS721 spec](../../packages/bs721/README.md) and is designed to
be deployed as is, or imported into other contracts to easily build
bs721-compatible NFTs with custom logic.

Implements:

- [x] BS721 Base

## Implementation

The `ExecuteMsg` and `QueryMsg` implementations follow the [BS721 spec](../../packages/bs721/README.md) and are described there.
Beyond that, we make a few additions:

* `InstantiateMsg` takes name, symbol (for metadata) and uri, as well as a **Minter** address. This is a special address that has full
power to mint new NFTs (but not modify existing ones)
* `ExecuteMsg::Mint{token_id, owner, token_uri}` - creates a new token with given owner and (optional) metadata. It can only be called by
the Minter set in `instantiate`.
* `QueryMsg::Minter{}` - returns the minter address for this contract.

It requires all tokens to have defined metadata in the standard format (with no extensions). For generic NFTs this may often be enough.

The *Minter* can either be an external actor (e.g. web server, using PubKey) or another contract. If you just want to customize
the minting behavior but not other functionality, you could extend this contract (importing code and wiring it together)
or just create a custom contract as the owner and use that contract to Mint.

If provided, it is expected that the _token_uri_ points to a JSON file following the [ERC721 Metadata JSON Schema](https://eips.ethereum.org/EIPS/eip-721).

## Running this contract

You will need Rust 1.44.1+ with `wasm32-unknown-unknown` target installed.

You can run unit tests on this via:

`cargo test`

Once you are happy with the content, you can compile it to wasm via:

```
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ../../target/wasm32-unknown-unknown/release/cw721_base.wasm .
ls -l cw721_base.wasm
sha256sum cw721_base.wasm
```

## Compiling

To compile all the contracts, run the following in the repo root:

```
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.12.9
```

This will compile all packages in the `contracts` directory and output the stripped and optimized wasm code under the
`artifacts` directory as output, along with a `checksums.txt` file.

If you hit any issues there and want to debug, you can try to run the following in each contract dir:
`RUSTFLAGS="-C link-arg=-s" cargo build --release --target=wasm32-unknown-unknown --locked`

## Importing this contract

You can also import much of the logic of this contract to build another
BS721-compliant contract, such as tradable names, crypto kitties,
or tokenized real estate.

Basically, you just need to write your handle function and import
`bs721_base::contract::handle_transfer`, etc and dispatch to them.
This allows you to use custom `ExecuteMsg` and `QueryMsg` with your additional
calls, but then use the underlying implementation for the standard bs721
messages you want to support. The same with `QueryMsg`. You will most
likely want to write a custom, domain-specific `instantiate`.
