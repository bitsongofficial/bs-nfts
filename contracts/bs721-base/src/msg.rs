use bs721::{Expiration, RoyaltyInfoResponse};
use bs_std::NATIVE_DENOM;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    coin, Addr, BankMsg, Binary, Event, Response, StdError, StdResult, SubMsg, Timestamp, Uint128,
};
use schemars::JsonSchema;

/// This is like Bs721ExecuteMsg but we add a Mint command for an owner
/// to make this stand-alone. You will likely want to remove mint and
/// use other control logic in any contract that inherits this.
#[cw_serde]
pub enum ExecuteMsg<T, E> {
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
    Mint(MintMsg<T>),

    /// Set a new minter
    SetMinter { new_minter: String },

    /// Burn an NFT the sender has access to
    Burn { token_id: String },

    /// Extension msg
    Extension { msg: E },
}

#[cw_serde]
pub struct MintMsg<T> {
    /// Unique ID of the NFT
    pub token_id: String,
    /// The owner of the newly minted NFT
    pub owner: String,
    /// Universal resource identifier for this NFT
    /// Should point to a JSON file that conforms to the ERC721
    /// Metadata JSON Schema
    pub token_uri: Option<String>,
    /// Seller fee basis points, 0-10000
    /// 0 means no fee, 100 means 1%, 10000 means 100%
    /// This is the fee paid by the buyer to the original creator
    pub seller_fee_bps: Option<u16>,
    /// Payment address, is the address that will receive the payment
    pub payment_addr: Option<String>,
    /// Any custom extension used by this contract
    pub extension: T,
}

#[cw_serde]
#[derive(QueryResponses)]
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

    #[returns(CollectionInfoResponse)]
    CollectionInfo {},

    /// Extension query
    #[returns(())]
    Extension { msg: Q },
}

/// Shows who can mint these tokens
#[cw_serde]
pub struct MinterResponse {
    pub minter: String,
}

#[cw_serde]
pub enum NftParams<T> {
    NftData {
        token_id: String,
        owner: String,
        token_uri: Option<String>,
        extension: T,
    },
}

#[cw_serde]
pub struct CollectionInfoResponse {
    pub creator: String,
    pub description: String,
    pub image: String,
    pub external_link: Option<String>,
    pub explicit_content: Option<bool>,
    pub start_trading_time: Option<Timestamp>,
    pub royalty_info: Option<RoyaltyInfoResponse>,
}

impl CollectionInfoResponse {
    pub fn royalty_payout(
        &self,
        collection: Addr,
        payment: Uint128,
        protocol_fee: Uint128,
        finders_fee: Option<Uint128>,
        res: &mut Response,
    ) -> StdResult<Uint128> {
        if let Some(royalty_info) = self.royalty_info.as_ref() {
            if royalty_info.share.is_zero() {
                return Ok(Uint128::zero());
            }
            let royalty = coin((payment * royalty_info.share).u128(), NATIVE_DENOM);
            if payment < (protocol_fee + finders_fee.unwrap_or(Uint128::zero()) + royalty.amount) {
                return Err(StdError::generic_err("Fees exceed payment"));
            }
            res.messages.push(SubMsg::new(BankMsg::Send {
                to_address: royalty_info.payment_address.to_string(),
                amount: vec![royalty.clone()],
            }));

            let event = Event::new("royalty-payout")
                .add_attribute("collection", collection.to_string())
                .add_attribute("amount", royalty.to_string())
                .add_attribute("recipient", royalty_info.payment_address.to_string());
            res.events.push(event);

            Ok(royalty.amount)
        } else {
            Ok(Uint128::zero())
        }
    }
}
