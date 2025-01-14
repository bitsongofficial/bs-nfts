use std::collections::HashSet;

use crate::ContractError;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Decimal, Uint128};

/// Represents a contributor to the collection.
#[cw_serde]
pub struct ContributorMsg {
    /// Contributor's role
    pub role: String,
    /// Amount of share associated to the contributor.
    pub shares: u32,
    /// Contributor's address.
    pub address: String,
}

#[cw_serde]
pub struct InstantiateMsg {
    /// Native denom distributed to contributors.
    pub denom: String,
    /// NFT collection contibutors.
    pub contributors: Vec<ContributorMsg>,
}

impl InstantiateMsg {
    /// Validates the contributor shares and computes the total shares.
    ///
    /// This function checks the following conditions:
    /// - Each contributor must have a non-zero share.
    /// - There should be no duplicate contributors.
    ///
    /// If all checks pass, the function returns the total shares as a `Uint128` value.
    pub fn validate_and_compute_total_shares(&mut self) -> Result<Uint128, ContractError> {
        // cannot instantiate a royality contract without at least one contributor
        const MAX_CONTRIBUTORS: u64 = 50;
        const MAX_CHARACTERS: u64 = 50;

        match self.contributors.len() {
            0 => return Err(ContractError::EmptyContributors {}),
            x if x > 50 => {
                return Err(ContractError::MaximumContributors {
                    max_contributors: MAX_CONTRIBUTORS,
                })
            }
            _ => (),
        }

        let mut addresses = Vec::with_capacity(self.contributors.len());
        let mut total_shares = Uint128::zero();
        for contributor in &self.contributors {
            if contributor.shares == 0 {
                return Err(ContractError::InvalidShares {});
            }
            if contributor.role.len() > 50 {
                return Err(ContractError::MaximumCharacters {
                    max_characters: MAX_CHARACTERS,
                });
            }
            addresses.push(contributor.address.clone());
            total_shares = total_shares.checked_add(Uint128::from(contributor.shares))?;
        }

        // check if contributor addresses are a set
        let addresses_as_set: HashSet<&String> = addresses.iter().collect();

        if addresses.len() != addresses_as_set.len() {
            return Err(ContractError::DuplicateContributor {});
        }

        Ok(total_shares)
    }
}

#[cw_serde]

pub enum ExecuteMsg {
    /// Update contributors withdrawable amount by computing each contributors percentage of the
    /// total distributable contract balance. This function will consider only coins of the stored denom.
    Distribute {},
    /// Withdraw accrued royalties. This message can only be sent by a contributor.
    Withdraw {},
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    /// Retrieves the list of contributors.
    #[returns(ContributorListResponse)]
    ListContributors {
        /// Address after which contributors are retrieved.
        start_after: Option<String>,
        /// Number of contributors to receive.
        limit: Option<u32>,
    },
    /// Returns the total amount of royalties that can be withdrawn from the contract.
    #[returns(Uint128)]
    WithdrawableAmount {},
    /// Retrieves amount of denom that can be distributed.
    #[returns(Uint128)]
    DistributableAmount {},
}

/// Retrieved contributors response.
#[cw_serde]
pub struct ContributorListResponse {
    pub contributors: Vec<ContributorResponse>,
}

/// Single contributor response info.
#[cw_serde]
pub struct ContributorResponse {
    /// Address of the contributor.
    pub address: String,
    /// Role of the contributor.
    pub role: String,
    /// Shares of the contributor.
    pub initial_shares: u32,
    /// Shares of the contributor in terms of percentage of total shares
    pub percentage_shares: Decimal,
    /// Amount of royalties that can be withdrawn
    pub withdrawable_royalties: Uint128,
}

// -------------------------------------------------------------------------------------------------
// Unit tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn validate_single() {
        let mut msg = InstantiateMsg {
            denom: "bitsong".to_owned(),
            contributors: vec![],
        };

        {
            let err = msg.validate_and_compute_total_shares().unwrap_err();
            assert_eq!(
                err,
                ContractError::EmptyContributors {},
                "expected to fail since at least one contributor is requried"
            )
        }

        {
            let contributor = ContributorMsg {
                role: String::from("dj"),
                shares: 0,
                address: String::from("bitsong0000"),
            };

            msg.contributors.push(contributor);

            let err = msg.validate_and_compute_total_shares().unwrap_err();
            assert_eq!(
                err,
                ContractError::InvalidShares {},
                "expected to fail since zero shares is not allowed"
            )
        }

        {
            let contributor = ContributorMsg {
                // 51 characters role
                role: String::from("Pizzaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                shares: 10,
                address: String::from("bitsong0000"),
            };

            msg = InstantiateMsg {
                contributors: vec![contributor],
                ..msg
            };

            let err = msg.validate_and_compute_total_shares().unwrap_err();
            assert_eq!(
                err,
                ContractError::MaximumCharacters { max_characters: 50 },
                "expected to fails since role has more than max allowed characters"
            )
        }

        {
            let contributor = ContributorMsg {
                role: String::from("dj"),
                shares: 10,
                address: String::from("bitsong0000"),
            };

            msg = InstantiateMsg {
                contributors: vec![contributor],
                ..msg
            };

            let total_shares = msg.validate_and_compute_total_shares().unwrap();
            assert_eq!(
                total_shares,
                Uint128::from(10u128),
                "expected to pass since valid msg"
            )
        }
    }

    #[test]
    pub fn validate_multiple() {
        let contributor_1 = ContributorMsg {
            role: String::from("dj"),
            shares: 10,
            address: String::from("bitsong0000"),
        };

        {
            let contributor_2 = ContributorMsg {
                role: String::from("dj"),
                shares: 10,
                address: String::from("bitsong0000"),
            };

            let mut msg = InstantiateMsg {
                denom: "bitsong".to_owned(),
                contributors: vec![contributor_1.clone(), contributor_2],
            };

            let val = msg.validate_and_compute_total_shares().unwrap_err();
            assert_eq!(
                val,
                ContractError::DuplicateContributor {},
                "expected to fail since duplicated contributors are not allowed"
            )
        }

        {
            let contributor_2 = ContributorMsg {
                role: String::from("drawer"),
                shares: 0,
                address: String::from("bitsong1111"),
            };

            let mut msg = InstantiateMsg {
                denom: "bitsong".to_owned(),
                contributors: vec![contributor_1.clone(), contributor_2],
            };

            let val = msg.validate_and_compute_total_shares().unwrap_err();
            assert_eq!(
                val,
                ContractError::InvalidShares {},
                "expected to fail since all contributors must have shares"
            )
        }

        {
            let mut contributors: Vec<ContributorMsg> = Vec::with_capacity(51);
            for i in 0..51 {
                contributors.push(ContributorMsg {
                    // 51 characters role
                    role: String::from("drawer"),
                    shares: 10,
                    address: format!("bitsong000{}", i),
                })
            }

            let mut msg = InstantiateMsg {
                contributors,
                denom: "bitsong".to_owned(),
            };

            let err = msg.validate_and_compute_total_shares().unwrap_err();
            assert_eq!(
                err,
                ContractError::MaximumContributors {
                    max_contributors: 50
                },
                "expected to fail since maximum number of contributors reached"
            )
        }

        {
            // we should check overflow error properly handled.
        }

        {
            let contributor_2 = ContributorMsg {
                role: String::from("drawer"),
                shares: 10,
                address: String::from("bitsong1111"),
            };

            let mut msg = InstantiateMsg {
                denom: "bitsong".to_owned(),
                contributors: vec![contributor_1, contributor_2],
            };

            let val = msg.validate_and_compute_total_shares().unwrap();
            assert_eq!(val, Uint128::from(20u128), "expected to pass")
        }
    }
}
