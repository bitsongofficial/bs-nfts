use bs721::Expiration;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Binary;
use cw_ownable::cw_ownable_execute;
use schemars::JsonSchema;

#[cw_serde]
pub struct InstantiateMsg {
    /// Name of the NFT contract
    pub name: String,
    /// Symbol of the NFT contract
    pub symbol: String,
    /// Uri, optional uri to get more information about the NFT
    pub uri: Option<String>,
    /// The minter is the only one who can create new NFTs.
    /// This is designed for a base NFT that is controlled by an external program
    /// or contract. You will likely replace this with custom logic in custom NFTs
    pub minter: String,
}

/// This is like Bs721ExecuteMsg but we add a Mint command for an owner
/// to make this stand-alone. You will likely want to remove mint and
/// use other control logic in any contract that inherits this.
#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg<T> {
    /// Transfer is a base message to move a token to another account without triggering actions
    TransferNft { recipient: String, token_id: String },
    /// Send is a base message to transfer a token to a contract and trigger an action
    /// on the receiving contract.
    SendNft {
        contract: String,
        token_id: String,
        msg: Binary,
    },
    /// Allows operator to transfer / send the token from the owner's account.
    /// If expiration is set, then this allowance has a time/height limit
    Approve {
        spender: String,
        token_id: String,
        expires: Option<Expiration>,
    },
    /// Remove previously granted Approval
    Revoke { spender: String, token_id: String },
    /// Allows operator to transfer / send any token from the owner's account.
    /// If expiration is set, then this allowance has a time/height limit
    ApproveAll {
        operator: String,
        expires: Option<Expiration>,
    },
    /// Remove previously granted ApproveAll permission
    RevokeAll { operator: String },

    /// Mint a new NFT, can only be called by the contract minter
    Mint {
        token_id: String,
        owner: String,
        token_uri: Option<String>,
        seller_fee_bps: Option<u16>,
        /// Payment address, is the address that will receive the payment
        payment_addr: Option<String>,
        /// Any custom extension used by this contract
        extension: T,
    },
    /// Set a new minter
    SetMinter { new_minter: String },

    /// Burn an NFT the sender has access to
    Burn { token_id: String },
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg<Q: JsonSchema> {
    /// Return the owner of the given token, error if token does not exist
    #[returns(bs721::OwnerOfResponse)]
    OwnerOf {
        token_id: String,
        /// unset or false will filter out expired approvals, you must set to true to see them
        include_expired: Option<bool>,
    },
    /// Return operator that can access all of the owner's tokens.
    #[returns(bs721::ApprovalResponse)]
    Approval {
        token_id: String,
        spender: String,
        include_expired: Option<bool>,
    },
    /// Return approvals that a token has
    #[returns(bs721::ApprovalsResponse)]
    Approvals {
        token_id: String,
        include_expired: Option<bool>,
    },
    /// List all operators that can access all of the owner's tokens
    #[returns(bs721::OperatorsResponse)]
    AllOperators {
        owner: String,
        /// unset or false will filter out expired items, you must set to true to see them
        include_expired: Option<bool>,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Total number of tokens issued
    #[returns(bs721::NumTokensResponse)]
    NumTokens {},

    /// With MetaData Extension.
    /// Returns top-level metadata about the contract
    #[returns(bs721::ContractInfoResponse)]
    ContractInfo {},
    /// With MetaData Extension.
    /// Returns metadata about one particular token, based on *ERC721 Metadata JSON Schema*
    /// but directly from the contract
    #[returns(bs721::NftInfoResponse<Q>)]
    NftInfo { token_id: String },
    /// With MetaData Extension.
    /// Returns the result of both `NftInfo` and `OwnerOf` as one query as an optimization
    /// for clients
    #[returns(bs721::AllNftInfoResponse<Q>)]
    AllNftInfo {
        token_id: String,
        /// unset or false will filter out expired approvals, you must set to true to see them
        include_expired: Option<bool>,
    },

    /// With Enumerable extension.
    /// Returns all tokens owned by the given address, [] if unset.
    #[returns(bs721::TokensResponse)]
    Tokens {
        owner: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// With Enumerable extension.
    /// Requires pagination. Lists all token_ids controlled by the contract.
    #[returns(bs721::TokensResponse)]
    AllTokens {
        start_after: Option<String>,
        limit: Option<u32>,
    },

    /// Return the minter
    #[returns(MinterResponse)]
    Minter {},

    // #[returns(CollectionInfoResponse)]
    // CollectionInfo {},

    /// Extension query
    #[returns(())]
    Extension { msg: Q },
}

/// Shows who can mint these tokens
#[cw_serde]
pub struct MinterResponse {
    pub minter: String,
}