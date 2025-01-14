use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin};
use cw_storage_plus::Item;

/// Smart contract configuration structure.
#[cw_serde]
pub struct Config {
    /// Address allowed to update contract parameters
    pub owner: Addr,
    /// Code id used to instantiate a bs721 metadata onchain token contract.
    pub bs721_code_id: u64,
    /// Code id used to instantiate bs721 royalties contract. The address of this contract will be used
    /// as the payment address for the NFT mint.
    pub bs721_royalties_code_id: u64,

    pub bs721_launchparty_code_id: u64,

    pub bs721_curve_code_id: u64,
    /// Protocol fee as basis points
    pub protocol_fee_bps: u32,

    pub create_nft_sale_fee: Coin,
}

/// Stores the contract's configuration
pub const CONFIG: Item<Config> = Item::new("config");
