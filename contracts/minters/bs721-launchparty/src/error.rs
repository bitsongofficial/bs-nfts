use cosmwasm_std::{StdError, Uint128};
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error("max editions cannot be zero")]
    ZeroEditions {},

    #[error("party duration cannot be zero")]
    ZeroDuration {},

    #[error("BS721 contract already linked")]
    Bs721BaseAlreadyLinked {},

    #[error("royalties contract already linked")]
    RoyaltiesAlreadyLinked {},

    #[error("BS721 contract not linked")]
    Bs721NotLinked {},

    #[error("Royalties contract not linked")]
    RoyaltiesNotLinked {},

    #[error("unknown reply id")]
    UnknownReplyId {},

    #[error("contract is sold out")]
    SoldOut {},

    #[error("coins sent are invalid")]
    InvalidFunds {},

    #[error("invalid payment amount. Sent is {0} but required is {1}")]
    InvalidPaymentAmount(Uint128, Uint128),

    #[error("launchpad not started")]
    NotStarted {},

    #[error("party has ended")]
    PartyEnded {},

    #[error("{profile} fee bps must be less than 10000")]
    FeeBps { profile: String },

    #[error("max number of pre-generated metadata reached")]
    MaxMetadataReached {},

    #[error("max number of mint, remaining: {remaining}")]
    MaxPerAddressExceeded { remaining: u32 },
}
