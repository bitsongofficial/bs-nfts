use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Timestamp};
use cw_storage_plus::{Item, Map};

use crate::msg::PartyType;

/// Smart contract configuration structure.
#[cw_serde]
pub struct Config {
    /// Creator of the collection. If not provided it will be the sender. The minter is the only one who can create new NFTs.
    pub creator: Addr,
    /// Name of the NFT contract
    pub name: String,
    /// Symbol of the NFT contract
    pub symbol: String,
    /// Price of single nft minting.
    pub price: Coin,
    /// Maximum amount of token an address can mint.
    pub max_per_address: Option<u32>,
    /// Uri, optional uri to get more information about the NFT
    pub base_token_uri: String,
    /// ID of the next NFT that will be minted. The first NFT will be minted with ID == 1.
    pub next_token_id: u32,
    pub seller_fee_bps: u16,
    pub referral_fee_bps: u16,
    /// Start time of the launchparty.
    pub start_time: Timestamp,
    /// End condition of the collection launchparty.
    pub party_type: PartyType,
    /// Address of the bs721 token contract.
    pub bs721_base_address: Option<Addr>,
    /// Address of the bs721 royalties contract.
    pub royalties_address: Option<Addr>,
}

/// Stores the contract's configuration
pub const CONFIG: Item<Config> = Item::new("config");
pub const ADDRESS_TOKENS: Map<&Addr, u32> = Map::new("address_tokens");
