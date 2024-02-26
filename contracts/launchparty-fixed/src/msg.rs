use bs721_royalties::msg::ContributorMsg;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Coin, Timestamp};

use crate::ContractError;

/// Possible launchparty type. Each type defines how the party end.
#[cw_serde]
pub enum PartyType {
    /// Maximum number of mintable tokens.
    MaxEdition(u32),
    /// Number of blocks for which tokens are mintable.
    Duration(u32),
}

/// Structure required by the launchparty-fixed contract during its instantiation.
#[cw_serde]
pub struct InstantiateMsg {
    /// Creator of the collection. If not provided it will be the sender.
    // pub creator: Option<String>,
    /// BS721 token name.
    pub name: String,
    /// BS721 token symbol.
    pub symbol: String,
    /// Price of single nft minting.
    pub price: Coin,
    /// BS721 token uri.
    pub base_token_uri: String,
    /// Maximum amount of tokens an address can mint.
    pub max_per_address: Option<u32>,
    /// BS721 collection uri.
    pub collection_uri: String,
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
    /// Code id used to instantiate a bs721 token contract.
    pub bs721_base_code_id: u64,
    /// Code id used to instantiate bs721 royalties contract. The address of this contract will be used
    /// as the payment address for the NFT mint.
    pub bs721_royalties_code_id: u64,
}

/// Possible state-changing messages that the launchparty-fixed contract can handle.
#[cw_serde]
pub enum ExecuteMsg {
    /// Allows to mint a bs721 token and, optionally, to refer an address.
    Mint {
        /// Amount of token to mint. The maximum number an address can mint can be limited by the field
        /// `maximum_per_address` defined in the `Config`.
        amount: u32,
        /// Referral address used for minting.
        referral: Option<String>,
    },
}

/// Possible query messages that the launchparty-fixed contract can handle.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Retrieves contract's configuration
    #[returns(ConfigResponse)]
    GetConfig {},

    /// Returns the maximum amount of token an address can mint.
    #[returns(MaxPerAddressResponse)]
    MaxPerAddress { address: String },
}

#[cw_serde]
pub struct ConfigResponse {
    /// Creator of the collection. If not provided it will be the sender.
    pub creator: Addr,
    /// Address of the bs721 token contract.
    pub bs721_base: Option<Addr>,
    /// Address of the bs721 royalties contract.
    pub bs721_royalties: Option<Addr>,
    /// Price of single nft minting.
    pub price: Coin,
    /// Maximum amount of token an address can mint.
    pub max_per_address: Option<u32>,
    /// BS721 token name.
    pub name: String,
    /// BS721 token symbol.
    pub symbol: String,
    pub base_token_uri: String,
    /// ID that will be associated to the next NFT minted.
    pub next_token_id: u32,
    pub seller_fee_bps: u16,
    /// BPS of the token price associated to the referral address.
    pub referral_fee_bps: u16,
    /// Start time of the launchparty.
    pub start_time: Timestamp,
    /// End condition of the collection launchparty.
    pub party_type: PartyType,
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
    /// - maximum bps allowed for both seller and referral.
    /// - end condition of the launchparty.
    pub fn validate(&self) -> Result<(), ContractError> {
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

        self.party_type.validate()?;

        Ok(())
    }
}

impl PartyType {
    /// Performs basic validation checks on the party type.
    ///
    /// # Validation Checks
    ///
    /// - the number of maximum edition cannot be zero.
    /// - or, the party cannot end in the same time of the instantiation.
    pub fn validate(&self) -> Result<(), ContractError> {
        match self {
            PartyType::MaxEdition(number) => {
                if number == &0u32 {
                    return Err(ContractError::ZeroEditions {});
                }
            }
            PartyType::Duration(duration) => {
                if duration == &0u32 {
                    return Err(ContractError::ZeroDuration {});
                }
            }
        }
        Ok(())
    }
}

// -------------------------------------------------------------------------------------------------
// Unit tests
// -------------------------------------------------------------------------------------------------
#[cfg(test)]
mod test {
    use cosmwasm_std::coin;

    use super::*;

    #[test]
    fn party_type_validate_works() {
        {
            let party_type = PartyType::Duration(0);
            let err = party_type.validate().unwrap_err();

            assert_eq!(
                err,
                ContractError::ZeroDuration {},
                "expected to fail since no zero duration party is allowed"
            );
        }

        {
            let party_type = PartyType::MaxEdition(0);
            let err = party_type.validate().unwrap_err();

            assert_eq!(
                err,
                ContractError::ZeroEditions {},
                "expected to fail since no party with zero editions is allowed"
            );
        }
    }

    #[test]
    fn instantiate_msg_validate_works() {
        let mut msg = InstantiateMsg {
            name: "Launchparty".to_string(),
            price: coin(1, "ubtsg"),
            //creator: Some(String::from("creator")),
            max_per_address: Some(100),
            symbol: "LP".to_string(),
            base_token_uri: "ipfs://Qm......".to_string(),
            collection_uri: "ipfs://Qm......".to_string(),
            seller_fee_bps: 100,
            referral_fee_bps: 1,
            contributors: vec![],
            start_time: Timestamp::from_seconds(0),
            party_type: PartyType::MaxEdition(1),
            bs721_royalties_code_id: 0,
            bs721_base_code_id: 1,
        };

        {
            msg.seller_fee_bps = 10_001;
            let err = msg.validate().unwrap_err();
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
            let err = msg.validate().unwrap_err();
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
