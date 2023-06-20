use cosmwasm_std::{StdError, OverflowError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error("duplicate contributor: [{contributor}]")]
    DuplicateContributor { contributor: String },

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
}