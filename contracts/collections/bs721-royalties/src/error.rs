use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error("duplicate contributor")]
    DuplicateContributor {},

    #[error("no funds to withdraw")]
    NoFunds {},

    #[error("empty contributors")]
    EmptyContributors {},

    #[error("invalid shares")]
    InvalidShares {},

    #[error("{0}")]
    OverflowErr(#[from] OverflowError),

    #[error("nothing to distribute")]
    NothingToDistribute {},

    #[error("not enough royalties to distribute")]
    NotEnoughToDistribute {},

    #[error("nothing to withdraw")]
    NothingToWithdraw {},

    #[error("maxmimum number of contirbutors is {max_contributors}")]
    MaximumContributors { max_contributors: u64 },

    #[error("maxmimum number of characters is {max_characters}")]
    MaximumCharacters { max_characters: u64 },
}
