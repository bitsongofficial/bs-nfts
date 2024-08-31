use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Timestamp};
use cw_storage_plus::{Item, Map};

/// Smart contract configuration structure.
#[cw_serde]
pub struct Config {
    /// Creator of the collection. If not provided it will be the sender. The minter is the only one who can create new NFTs.
    pub creator: Addr,
    /// Symbol of the NFT contract
    pub symbol: String,
    /// Name of the NFT contract
    pub name: String,
    /// URI of the NFT contract
    pub uri: String,
    /// Denom used to pay for the NFTs
    pub payment_denom: String,
    /// Maximum amount of token an address can mint.
    pub max_per_address: Option<u32>,
    /// ID of the next NFT that will be minted. The first NFT will be minted with ID == 1.
    pub next_token_id: u32,
    pub seller_fee_bps: u16,
    pub referral_fee_bps: u16,
    pub protocol_fee_bps: u16,
    /// Start time of the launchparty.
    pub start_time: Timestamp,
    /// Max edition of the collection launchparty.
    pub max_edition: Option<u32>,
    /// Address of the bs721 metadata-onchain token contract.
    pub bs721_address: Option<Addr>,
    /// Address of the bs721 royalties contract.
    pub payment_address: Addr,
    /// Ratio, is the cooeficient of the curve
    pub ratio: u32,
}

/// Stores the contract's configuration
pub const CONFIG: Item<Config> = Item::new("config");
pub const ADDRESS_TOKENS: Map<&Addr, u32> = Map::new("address_tokens");

#[cw_serde]
pub struct Trait {
    pub display_type: Option<String>,
    pub trait_type: String,
    pub value: String,
}

#[cw_serde]
pub struct EditionMetadata {
    pub name: String,
    pub attributes: Option<Vec<Trait>>,
}
