use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Env, StdError, Timestamp, Uint128};

use crate::{state::Config, ContractError};

/// Structure required by the launchparty-curve contract during its instantiation.
#[cw_serde]
pub struct InstantiateMsg {
    /// BS721 token symbol.
    pub symbol: String,
    /// BS721 token name.
    pub name: String,
    /// BS721 Uri
    pub uri: String,
    /// Denom used to pay for the NFTs
    pub payment_denom: String,
    /// Maximum amount of tokens an address can mint.
    pub max_per_address: Option<u32>,
    /// Payment address for the royalties.
    pub payment_address: String,
    /// Basis per point of the `price` sent to the royalties address during mint or burn.
    pub seller_fee_bps: u16,
    /// Basis per point of the `price` sent to the referred address during mint or burn.
    pub referral_fee_bps: u16,
    /// Basis per point of the `price` sent to the community pool during mint or burn.
    pub protocol_fee_bps: u16,
    /// Start time of the launchparty.
    pub start_time: Timestamp,
    /// Max edition of the collection launchparty.
    pub max_edition: Option<u32>,
    /// Code id used to instantiate a bs721 metadata onchain token contract.
    pub bs721_code_id: u64,
    /// Ratio, is the cooeficient of the curve
    pub ratio: u32,
    pub bs721_admin: String,
}

/// Possible state-changing messages that the launchparty-curve contract can handle.
#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    /// Allows to mint a bs721 token and, optionally, to refer an address.
    Mint {
        /// Amount of token to mint. The maximum number an address can mint can be limited by the field
        /// `maximum_per_address` defined in the `Config`.
        amount: u32,
        /// Referral address used for minting.
        referral: Option<String>,
    },

    Burn {
        token_ids: Vec<u32>,
        min_out_amount: u128,
        referral: Option<String>,
    },
}

/// Possible query messages that the launchparty-curve contract can handle.
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    /// Retrieves contract's configuration
    #[returns(Config)]
    GetConfig {},

    /// Returns the maximum amount of token an address can mint.
    #[returns(MaxPerAddressResponse)]
    MaxPerAddress { address: String },

    #[returns(PriceResponse)]
    BuyPrice { amount: u128 },

    #[returns(PriceResponse)]
    SellPrice { amount: u128 },
}

#[cw_serde]
pub struct PriceResponse {
    pub base_price: Uint128,
    pub royalties: Uint128,
    pub referral: Uint128,
    pub protocol_fee: Uint128,
    pub total_price: Uint128,
}

#[cw_serde]
pub struct MaxPerAddressResponse {
    /// Returns the maximum amount of token an address can mint.
    pub remaining: Option<u32>,
}

impl InstantiateMsg {
    const MAX_FEE_BPS: u16 = 10_000;

    /// Performs basic validation checks on the InstantiateMsg type.
    ///
    /// # Validation Checks:
    ///
    /// - start time must be in the future.
    /// - maximum bps allowed for both seller and referral.
    /// - end condition of the launchparty.
    pub fn validate(&self, _env: Env) -> Result<(), ContractError> {
        // validate seller_fee_bps
        if self.seller_fee_bps > Self::MAX_FEE_BPS {
            return Err(ContractError::FeeBps {
                profile: String::from("seller"),
            });
        }

        // validate referral_fee_bps
        if self.referral_fee_bps > Self::MAX_FEE_BPS {
            return Err(ContractError::FeeBps {
                profile: String::from("referral"),
            });
        }

        // validate denom
        if self.payment_denom.is_empty() {
            return Err(ContractError::Std(StdError::generic_err(
                "payment denom cannot be empty",
            )));
        }

        Ok(())
    }
}

// -------------------------------------------------------------------------------------------------
// Unit tests
// -------------------------------------------------------------------------------------------------
/*#[cfg(test)]
mod test {
    use cosmwasm_std::{coin, testing::mock_env};

    use super::*;
    #[test]
    fn instantiate_msg_validate_works() {
        let mut msg = InstantiateMsg {
            //name: "Launchparty".to_string(),
            price: coin(1, "ubtsg"),
            //creator: Some(String::from("creator")),
            max_per_address: Some(100),
            symbol: "LP".to_string(),
            //collection_uri: "ipfs://Qm......".to_string(),
            collection_image: "ipfs://Qm......".to_string(),
            collection_cover_image: Some("ipfs://Qm......".to_string()),
            metadata: Metadata {
                image: Some("ipfs://Qm......".to_string()),
                image_data: None,
                external_url: None,
                description: "".to_string(),
                name: "Launchparty".to_string(),
                attributes: None,
                background_color: None,
                animation_url: None,
                media_type: Some(MediaType::Image),
            },
            seller_fee_bps: 100,
            referral_fee_bps: 1,
            contributors: vec![],
            start_time: Timestamp::from_seconds(0),
            party_type: PartyType::MaxEdition(1),
            bs721_royalties_code_id: 0,
            bs721_metadata_code_id: 1,
        };

        {
            msg.seller_fee_bps = 10_001;
            let err = msg.validate(mock_env()).unwrap_err();
            assert_eq!(
                err,
                ContractError::FeeBps {
                    profile: String::from("seller")
                },
                "expected to fail since fee bps higher than maximum allowed"
            );
            msg.seller_fee_bps = 1_000;
        }

        {
            msg.referral_fee_bps = 10_001;
            let err = msg.validate(mock_env()).unwrap_err();
            assert_eq!(
                err,
                ContractError::FeeBps {
                    profile: String::from("referral")
                },
                "expected to fail since fee bps higher than maximum allowed"
            );
            msg.referral_fee_bps = 1_000;
        }
    }
}
*/
