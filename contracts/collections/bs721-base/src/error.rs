use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("token_id already claimed")]
    Claimed {},

    #[error("Cannot set approval that is already expired")]
    Expired {},

    #[error("Approval not found for: {spender}")]
    ApprovalNotFound { spender: String },

    #[error("Max seller fee exceeded 10000")]
    MaxSellerFeeExceeded {},

    #[error("Seller fee and payment address must be set together")]
    InvalidSellerFee {},
    
    #[error(transparent)]
    Ownership(#[from] cw_ownable::OwnershipError),
}
