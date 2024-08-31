use bs721_royalties::msg::ContributorMsg;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Env, Timestamp};

use bs721_launchparty::msg::PartyType;

use crate::{state::Config, ContractError};

#[cw_serde]
pub struct InstantiateMsg {
    /// Address allowed to update contract parameters
    pub owner: String,
    /// Code id used to instantiate a bs721 metadata onchain token contract.
    pub bs721_code_id: u64,
    /// Code id used to instantiate bs721 royalties contract. The address of this contract will be used
    /// as the payment address for the NFT mint.
    pub bs721_royalties_code_id: u64,
    /// Code id of bs721-launchparty contract
    pub bs721_launchparty_code_id: u64,
    /// Code id of bs721-curve
    pub bs721_curve_code_id: u64,
    /// Protocol fee as basis points
    pub protocol_fee_bps: u32,
    /// Required asset to create a instance using this factory.
    pub create_nft_sale_fee: Coin,
}

#[cw_serde]
pub struct MsgCreateLaunchparty {
    /// BS721 token symbol.
    pub symbol: String,
    /// BS721 token name.
    pub name: String,
    /// BS721 Uri
    pub uri: String,
    /// Price of single nft minting.
    pub price: Coin,
    /// Maximum amount of tokens an address can mint.
    pub max_per_address: Option<u32>,
    /// Basis per point of the `price` sent to the referred address during mint. This payment is sent
    /// one-off.
    pub seller_fee_bps: u16,
    /// Basis per point of the `price` sent to the referred address during mint. This payment is sent
    /// one-off.
    pub referral_fee_bps: u16,
    /// Contributors to the collection.
    pub contributors: Vec<ContributorMsg>,
    /// Start time of the launchparty.
    pub start_time: Timestamp,
    /// End condition of the collection launchparty.
    pub party_type: PartyType,
    /// Address to recieve payments from launchparty.
    pub payment_address: String,
}

#[cw_serde]
pub struct MsgCreateCurve {
    /// BS721 token symbol.
    pub symbol: String,
    /// BS721 token name.
    pub name: String,
    /// BS721 Uri
    pub uri: String,
    /// Denom used to pay for the NFTs
    pub payment_denom: String,
    pub payment_address: String,
    /// Maximum amount of tokens an address can mint.
    pub max_per_address: Option<u32>,
    /// Basis per point of the `price` sent to the referred address during mint. This payment is sent
    /// one-off.
    pub seller_fee_bps: u16,
    /// Basis per point of the `price` sent to the referred address during mint. This payment is sent
    /// one-off.
    pub referral_fee_bps: u16,
    /// Start time of the launchparty.
    pub start_time: Timestamp,
    /// Max edition of the collection launchparty.
    pub max_edition: Option<u32>,
    /// Ratio, is the cooeficient of the curve
    pub ratio: u32,
}

#[cw_serde]
pub enum ExecuteMsg {
    UpdateConfig {
        /// Address allowed to update contract parameters
        owner: Option<String>,
        /// Code id used to instantiate a bs721 metadata onchain token contract.
        bs721_code_id: Option<u64>,
        /// Code id used to instantiate bs721 royalties contract. The address of this contract will be used
        /// as the payment address for the NFT mint.
        bs721_royalties_code_id: Option<u64>,

        bs721_launchparty_code_id: Option<u64>,

        bs721_curve_code_id: Option<u64>,
        /// Protocol fee as basis points
        protocol_fee_bps: Option<u32>,

        create_nft_sale_fee: Option<Coin>,
    },

    CreateLaunchaparty(MsgCreateLaunchparty),

    CreateCurve(MsgCreateCurve),

    CreateRoyaltiesGroup {
        /// Native denom distributed to contributors.
        denom: String,
        /// NFT collection contibutors.
        contributors: Vec<ContributorMsg>,
    },
}

/// Possible query messages that the launchparty-curve contract can handle.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Retrieves contract's configuration
    #[returns(Config)]
    Config {},
}

impl InstantiateMsg {
    const MAX_PROTOCOL_FEE_BPS: u32 = 1_000; // 10%

    pub fn validate(&self, _env: Env) -> Result<(), ContractError> {
        // validate protocol_fee_bps
        if self.protocol_fee_bps > Self::MAX_PROTOCOL_FEE_BPS {
            return Err(ContractError::FeeBps {
                profile: String::from("protocol"),
            });
        }

        Ok(())
    }
}
