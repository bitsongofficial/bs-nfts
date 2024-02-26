use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MaxPerAddressResponse, PartyType, QueryMsg,
};
use crate::state::{Config, ADDRESS_TOKENS, CONFIG};

use bs721_base::{Extension, MintMsg};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, coin, to_binary, Addr, Attribute, BankMsg, Binary, Deps, DepsMut, Empty, Env,
    MessageInfo, Reply, ReplyOn, Response, StdError, StdResult, SubMsg, Timestamp, Uint128,
    WasmMsg,
};
use cw2::set_contract_version;

use bs721_base::msg::{
    ExecuteMsg as Bs721BaseExecuteMsg, InstantiateMsg as Bs721BaseInstantiateMsg,
};
use bs721_royalties::msg::InstantiateMsg as Bs721RoyaltiesInstantiateMsg;

use cw_utils::{may_pay, parse_reply_instantiate_data};

const CONTRACT_NAME: &str = "crates.io:launchparty-fixed";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// ID used to recognize the instantiate token reply in the reply entry point.
const INSTANTIATE_TOKEN_REPLY_ID: u64 = 1;
/// ID used to recognize the instantiate the royalties contract reply in the reply entry point.
const INSTANTIATE_ROYALTIES_REPLY_ID: u64 = 2;
/// Maximum tokens that can be minted in both cases of the `PartyType`.
// TODO: investigate how this can be removed by adding metadata to NFTs.
const OVERAL_MAXIMUM_MINTABLE: u32 = 10_000;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    msg.validate()?;

    let denom = msg.price.denom.clone();

    let config = Config {
        //creator: deps
        //    .api
        //    .addr_validate(&msg.creator.unwrap_or_else(|| info.sender.to_string()))?, // creator is in instantiate message or the sender
        creator: info.sender,
        name: msg.name.clone(),
        symbol: msg.symbol.clone(),
        base_token_uri: msg.base_token_uri.clone(),
        price: msg.price,
        max_per_address: msg.max_per_address,
        bs721_base_address: None,
        next_token_id: 1, // first token ID is 1
        seller_fee_bps: msg.seller_fee_bps,
        referral_fee_bps: msg.referral_fee_bps,
        royalties_address: None,
        start_time: msg.start_time,
        party_type: msg.party_type,
    };

    CONFIG.save(deps.storage, &config)?;

    // create submessages to instantiate token and royalties contracts
    let sub_msgs: Vec<SubMsg> = vec![
        SubMsg {
            id: INSTANTIATE_TOKEN_REPLY_ID,
            msg: WasmMsg::Instantiate {
                code_id: msg.bs721_base_code_id,
                msg: to_binary(&Bs721BaseInstantiateMsg {
                    name: msg.name.clone(),
                    symbol: msg.symbol.clone(),
                    minter: env.contract.address.to_string(),
                    uri: Some(msg.collection_uri.to_string()),
                })?,
                label: "Launchparty fixed contract".to_string(),
                admin: None,
                funds: vec![],
            }
            .into(),
            gas_limit: None,
            reply_on: ReplyOn::Success,
        },
        SubMsg {
            id: INSTANTIATE_ROYALTIES_REPLY_ID,
            msg: WasmMsg::Instantiate {
                code_id: msg.bs721_royalties_code_id,
                msg: to_binary(&Bs721RoyaltiesInstantiateMsg {
                    denom,
                    contributors: msg.contributors,
                })?,
                label: "Launchparty royalties contract".to_string(),
                admin: None,
                funds: vec![],
            }
            .into(),
            gas_limit: None,
            reply_on: ReplyOn::Success,
        },
    ];

    Ok(Response::new().add_submessages(sub_msgs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    let mut res = Response::new();

    let reply_res = parse_reply_instantiate_data(reply.clone()).map_err(|_| {
        StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
    })?;

    match reply.id {
        INSTANTIATE_TOKEN_REPLY_ID => {
            if config.bs721_base_address.is_some() {
                return Err(ContractError::Bs721BaseAlreadyLinked {});
            }

            config.bs721_base_address = Addr::unchecked(reply_res.contract_address.clone()).into();

            res = res
                .add_attribute("action", "bs721_base_reply")
                .add_attribute("contract_address", reply_res.contract_address)
        }
        INSTANTIATE_ROYALTIES_REPLY_ID => {
            if config.royalties_address.is_some() {
                return Err(ContractError::RoyaltiesAlreadyLinked {});
            }

            config.royalties_address = Addr::unchecked(reply_res.contract_address.clone()).into();

            res = res
                .add_attribute("action", "royalties_reply")
                .add_attribute("contract_address", reply_res.contract_address)
        }
        _ => return Err(ContractError::UnknownReplyId {}),
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Mint { amount, referral } => {
            // check if referral address is valid
            let referral = referral
                .map(|address| deps.api.addr_validate(address.as_str()))
                .transpose()?;
            execute_mint(deps, env, info, amount, referral)
        }
    }
}

fn execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: u32,
    referral: Option<Addr>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    let accepted_denom = config.price.denom.clone();

    before_mint_checks(&env, &config, amount)?;

    let already_minted = (ADDRESS_TOKENS.key(&info.sender).may_load(deps.storage)?).unwrap_or(0);
    let new_total_mint = already_minted.checked_add(amount).unwrap_or(1);

    if let Some(max_per_address) = config.max_per_address {
        if new_total_mint > max_per_address {
            return Err(ContractError::MaxPerAddressExceeded {
                remaining: max_per_address.checked_sub(already_minted).unwrap_or(0),
            });
        }
    }

    // check that the user has sent exactly the required amount. The amount is given by the price of
    // a single token times the number of tokens to mint.
    let sent_amount = may_pay(&info, &accepted_denom)?;
    let required_amount = config
        .price
        .amount
        .checked_mul(Uint128::from(amount))
        .map_err(StdError::overflow)?;
    if sent_amount != required_amount {
        return Err(ContractError::InvalidPaymentAmount(
            sent_amount,
            required_amount,
        ));
    }

    let mut res = Response::new();

    // create minting message
    for _ in 0..amount {
        let token_id = config.next_token_id;
        let token_uri = format!("{}/{}", config.base_token_uri.clone(), token_id);

        let mint_msg = Bs721BaseExecuteMsg::<Extension, Empty>::Mint(MintMsg::<Extension> {
            owner: info.sender.to_string(),
            token_id: token_id.to_string(),
            token_uri: Some(token_uri),
            extension: None,
            payment_addr: Some(config.royalties_address.clone().unwrap().to_string()),
            seller_fee_bps: Some(config.seller_fee_bps),
        });

        let msg = WasmMsg::Execute {
            contract_addr: config.bs721_base_address.clone().unwrap().to_string(),
            msg: to_binary(&mint_msg)?,
            funds: vec![],
        };

        res = res
            .add_message(msg)
            .add_attribute("token_id", token_id.to_string());

        config.next_token_id += 1;
        CONFIG.save(deps.storage, &config)?;
    }

    // create  royalties and optionally referral messages

    // if token price is not zero we have to send:
    // - referral bps * price to referred address.
    // - price - (referral bps * price) to royalties address
    if !config.price.amount.is_zero() {
        let (referral_amount, royalties_amount) =
            compute_referral_and_royalties_amounts(&config, &referral, required_amount)?;

        let mut bank_msgs: Vec<BankMsg> = vec![];
        let mut attributes: Vec<Attribute> = vec![];

        if !referral_amount.is_zero() {
            bank_msgs.push(BankMsg::Send {
                to_address: referral.clone().unwrap().to_string(),
                amount: vec![coin(referral_amount.u128(), accepted_denom.clone())],
            });

            attributes.push(attr("referral", referral.unwrap().to_string()));
            attributes.push(attr(
                "amount",
                coin(referral_amount.u128(), accepted_denom.clone()).to_string(),
            ));
        }

        bank_msgs.push(BankMsg::Send {
            to_address: config.royalties_address.clone().unwrap().to_string(),
            amount: vec![coin(royalties_amount.u128(), accepted_denom.clone())],
        });

        attributes.push(attr(
            "royalties",
            coin(royalties_amount.u128(), accepted_denom).to_string(),
        ));

        res = res.add_messages(bank_msgs).add_attributes(attributes)
    }

    CONFIG.save(deps.storage, &config)?;
    ADDRESS_TOKENS.save(deps.storage, &info.sender, &new_total_mint)?;

    Ok(res
        .add_attribute("action", "mint_launchparty_nft")
        .add_attribute("price", config.price.amount)
        .add_attribute("creator", config.creator.to_string())
        .add_attribute("recipient", info.sender.to_string()))
}

/// Computes the amount of `total_amount` associated with the referral address, if any, and the amount
/// associated with the royalties contract. If `referral` is None, zero tokens are associated with the referrer.
///
/// # Arguments
///
/// * `config` - Configuration parameters for the computation.
/// * `referral` - Optional referral address.
/// * `total_amount` - Total amount of tokens.
///
/// # Returns
///
/// Returns a tuple containing the referral amount and royalties amount as `Uint128`.
///
/// # Errors
///
/// Returns an error if an overflow occurs during the computation.
pub fn compute_referral_and_royalties_amounts(
    config: &Config,
    referral: &Option<Addr>,
    total_amount: Uint128,
) -> StdResult<(Uint128, Uint128)> {
    let referral_amount = referral.as_ref().map_or_else(
        || Ok(Uint128::zero()),
        |_address| -> Result<Uint128, _> {
            total_amount
                .checked_mul(Uint128::from(config.referral_fee_bps))
                .map_err(StdError::overflow)?
                .checked_div(Uint128::new(10_000))
                .map_err(StdError::divide_by_zero)
        },
    )?;

    let royalties_amount = total_amount - referral_amount;

    Ok((referral_amount, royalties_amount))
}

/// Basic checks performed before minting a token
///
/// ## Validation Checks
///
/// - start time older than current time.
/// - bs721 base address is stored in the contract.
/// - royalties address is stored in the contract.
/// - checks if party is active.
/// - check that maximum number of pre-generated metadata is not reched.
pub fn before_mint_checks(
    env: &Env,
    config: &Config,
    edition_to_mint: u32,
) -> Result<(), ContractError> {
    if config.start_time > env.block.time {
        return Err(ContractError::NotStarted {});
    }

    if config.bs721_base_address.is_none() {
        return Err(ContractError::Bs721NotLinked {});
    }

    if config.royalties_address.is_none() {
        return Err(ContractError::RoyaltiesNotLinked {});
    }

    if !party_is_active(
        env,
        &config.party_type,
        (config.next_token_id - 1) + edition_to_mint,
        config.start_time,
    ) {
        return Err(ContractError::PartyEnded {});
    }

    // TODO: remove this check
    if (config.next_token_id - 1) + edition_to_mint > OVERAL_MAXIMUM_MINTABLE {
        return Err(ContractError::MaxMetadataReached {});
    }

    Ok(())
}

/// Returns true if the launcharty is active, false otherwise.
///
/// A party is active if:
///
/// - maxmimum number of editions have been not already minted.
/// - current time is less than starting time plus party duration.
pub fn party_is_active(
    env: &Env,
    party_type: &PartyType,
    token_id: u32,
    start_time: Timestamp,
) -> bool {
    match party_type {
        PartyType::MaxEdition(number) => {
            if token_id > *number {
                return false;
            }
        }
        PartyType::Duration(duration) => {
            if start_time.plus_seconds(*duration as u64) < env.block.time {
                return false;
            }
        }
    }
    true
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_binary(&query_config(deps)?),
        QueryMsg::MaxPerAddress { address } => to_binary(&query_max_per_address(deps, address)?),
    }
}

fn query_max_per_address(deps: Deps, address: String) -> StdResult<MaxPerAddressResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let already_minted = (ADDRESS_TOKENS.key(&addr).may_load(deps.storage)?).unwrap_or(0);

    let config: Config = CONFIG.load(deps.storage)?;

    if let Some(max_per_address) = config.max_per_address {
        return Ok(MaxPerAddressResponse {
            remaining: Some(max_per_address.checked_sub(already_minted).unwrap_or(0)),
        });
    }

    Ok(MaxPerAddressResponse { remaining: None })
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        creator: config.creator,
        bs721_base: config.bs721_base_address,
        bs721_royalties: config.royalties_address,
        price: config.price,
        max_per_address: config.max_per_address,
        name: config.name,
        symbol: config.symbol,
        base_token_uri: config.base_token_uri,
        next_token_id: config.next_token_id,
        seller_fee_bps: config.seller_fee_bps,
        referral_fee_bps: config.referral_fee_bps,
        start_time: config.start_time,
        party_type: config.party_type,
    })
}

// -------------------------------------------------------------------------------------------------
// Unit test
// -------------------------------------------------------------------------------------------------
#[cfg(test)]
mod tests {

    use super::*;
    use bs721_royalties::msg::ContributorMsg;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{from_binary, to_binary, SubMsgResponse, SubMsgResult, Timestamp};
    use prost::Message;

    const NFT_CONTRACT_ADDR: &str = "nftcontract";
    const ROYALTIES_CONTRACT_ADDR: &str = "royaltiescontract";
    const BS721_BASE_CODE_ID: u64 = 1;
    const BS721_ROYALTIES_CODE_ID: u64 = 2;

    // Type for replies to contract instantiate messes
    #[derive(Clone, PartialEq, Message)]
    struct MsgInstantiateContractResponse {
        #[prost(string, tag = "1")]
        pub contract_address: ::prost::alloc::string::String,
        #[prost(bytes, tag = "2")]
        pub data: ::prost::alloc::vec::Vec<u8>,
    }

    #[test]
    fn party_is_active_works() {
        let env = mock_env();

        {
            let curr_block_time = env.block.time;
            let mut start_time = curr_block_time.minus_seconds(1);
            assert!(
                party_is_active(&env, &PartyType::Duration(1), 1, start_time),
                "expected true since current time equal to start time + party duration is still valid for minting"
            );

            start_time = curr_block_time.minus_seconds(2);
            assert!(
                !party_is_active(&env, &PartyType::Duration(1), 1, start_time),
                "expected false since current time 1s less then start time + party duration"
            )
        }
    }

    #[test]
    fn before_mint_checks_works() {
        let env = mock_env();

        let mut config = Config {
            creator: Addr::unchecked("creator"),
            name: String::from(""),
            symbol: String::from(""),
            price: coin(1, "ubtsg"),
            max_per_address: None,
            base_token_uri: String::from(""),
            next_token_id: 1,
            seller_fee_bps: 1_000,
            referral_fee_bps: 1_000,
            start_time: Timestamp::from_seconds(1),
            party_type: PartyType::MaxEdition(2),
            bs721_base_address: Some(Addr::unchecked("contract1")),
            royalties_address: Some(Addr::unchecked("contract2")),
        };

        {
            config.start_time = env.block.time.plus_seconds(1);
            let resp = before_mint_checks(&env, &config, 1).unwrap_err();
            assert_eq!(
                resp,
                ContractError::NotStarted {},
                "expected to fail since start time > current time"
            );
            config.start_time = env.block.time.minus_seconds(1);
        }

        {
            config.bs721_base_address = None;
            let resp = before_mint_checks(&env, &config, 1).unwrap_err();
            assert_eq!(
                resp,
                ContractError::Bs721NotLinked {},
                "expected to fail since cw721 base contract not linked"
            );
            config.bs721_base_address = Some(Addr::unchecked("contract1"));
        }

        {
            config.royalties_address = None;
            let resp = before_mint_checks(&env, &config, 1).unwrap_err();
            assert_eq!(
                resp,
                ContractError::RoyaltiesNotLinked {},
                "expected to fail since royalties contract not linked"
            );
            config.royalties_address = Some(Addr::unchecked("contract2"));
        }

        {
            // PartyType type has already tests, here we check for the error raised.
            config.party_type = PartyType::Duration(0);
            config.start_time = env.block.time.minus_seconds(1);
            let resp = before_mint_checks(&env, &config, 1).unwrap_err();
            assert_eq!(
                resp,
                ContractError::PartyEnded {},
                "expected to fail since party is ended"
            );
        }

        {
            config.party_type = PartyType::Duration(1);
            config.start_time = env.block.time.minus_seconds(1);
            config.next_token_id = OVERAL_MAXIMUM_MINTABLE + 1;
            let resp = before_mint_checks(&env, &config, 1).unwrap_err();
            assert_eq!(
                resp,
                ContractError::MaxMetadataReached {},
                "expected to fail since next token id is higher than overal maximum mintable tokens"
            );
        }
    }

    #[test]
    fn compute_referral_and_royalties_amounts_works() {
        let config = Config {
            creator: Addr::unchecked("creator"),
            name: String::from(""),
            symbol: String::from(""),
            price: coin(1, "ubtsg"),
            max_per_address: None,
            base_token_uri: String::from(""),
            next_token_id: 1,
            seller_fee_bps: 1_000,
            referral_fee_bps: 1_000,
            start_time: Timestamp::from_seconds(1),
            party_type: PartyType::MaxEdition(2),
            bs721_base_address: Some(Addr::unchecked("contract1")),
            royalties_address: Some(Addr::unchecked("contract2")),
        };

        {
            let (referral_amt, royalties_amt) =
                compute_referral_and_royalties_amounts(&config, &None, Uint128::new(1_000))
                    .unwrap();
            assert_eq!(
                Uint128::zero(),
                referral_amt,
                "expected zero referral amount since no referral address"
            );
            assert_eq!(
                Uint128::new(1_000),
                royalties_amt,
                "expected royalties amount equal to total amount"
            );
        }

        {
            let (referral_amt, royalties_amt) = compute_referral_and_royalties_amounts(
                &config,
                &Some(Addr::unchecked("referrral".to_string())),
                Uint128::new(1_000),
            )
            .unwrap();
            assert_eq!(
                Uint128::new(100),
                referral_amt,
                "expected 10% as referral amount"
            );
            assert_eq!(
                Uint128::new(900),
                royalties_amt,
                "expected 90% as royalties amount"
            );
        }

        {
            let (referral_amt, royalties_amt) = compute_referral_and_royalties_amounts(
                &config,
                &Some(Addr::unchecked("referrral".to_string())),
                Uint128::zero(),
            )
            .unwrap();
            assert_eq!(
                Uint128::new(0),
                referral_amt,
                "expected zero since zero amount"
            );
            assert_eq!(
                Uint128::new(0),
                royalties_amt,
                "expected zero since zero amount"
            );
        }

        {
            let (referral_amt, royalties_amt) = compute_referral_and_royalties_amounts(
                &config,
                &Some(Addr::unchecked("referrral".to_string())),
                Uint128::new(1),
            )
            .unwrap();
            assert_eq!(
                Uint128::zero(),
                referral_amt,
                "expected zero 10% of 1 is rounded zero"
            );
            assert_eq!(
                Uint128::new(1),
                royalties_amt,
                "expected 1 since royalties is 1 minus referral amount"
            );
        }

        {
            let (referral_amt, royalties_amt) = compute_referral_and_royalties_amounts(
                &config,
                &Some(Addr::unchecked("referrral".to_string())),
                Uint128::new(9),
            )
            .unwrap();
            assert_eq!(
                Uint128::zero(),
                referral_amt,
                "expected zero since 10% of 9 is rounded zero"
            );
            assert_eq!(
                Uint128::new(9),
                royalties_amt,
                "expected 9 since royalties is 9 minus referral amount"
            );
        }

        {
            let (referral_amt, royalties_amt) = compute_referral_and_royalties_amounts(
                &config,
                &Some(Addr::unchecked("referrral".to_string())),
                Uint128::new(10),
            )
            .unwrap();
            assert_eq!(Uint128::new(1), referral_amt, "expected 1 since 10% of 10");
            assert_eq!(
                Uint128::new(9),
                royalties_amt,
                "expected 9 since royalties is 10 minus referral amount"
            );
        }
    }

    #[test]
    fn initialization_fails() {
        let mut deps = mock_dependencies();

        let contributors = vec![ContributorMsg {
            address: "contributor".to_string(),
            role: String::from("creator"),
            shares: 100,
        }];

        let msg = InstantiateMsg {
            name: "Launchparty".to_string(),
            price: coin(1, "ubtsg"),
            max_per_address: Some(1),
            // creator: Some(String::from("creator")),
            symbol: "LP".to_string(),
            base_token_uri: "ipfs://Qm......".to_string(),
            collection_uri: "ipfs://Qm......".to_string(),
            seller_fee_bps: 100,
            referral_fee_bps: 1,
            contributors,
            start_time: Timestamp::from_seconds(0),
            party_type: PartyType::MaxEdition(1),
            bs721_royalties_code_id: BS721_ROYALTIES_CODE_ID,
            bs721_base_code_id: BS721_BASE_CODE_ID,
        };

        let info = mock_info("creator", &[]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    fn initialization() {
        let mut deps = mock_dependencies();

        let contributors = vec![ContributorMsg {
            address: "contributor".to_string(),
            role: String::from("creator"),
            shares: 100,
        }];

        let msg = InstantiateMsg {
            name: "Launchparty".to_string(),
            price: coin(1, "ubtsg"),
            max_per_address: Some(1),
            // creator: Some(String::from("creator")),
            symbol: "LP".to_string(),
            base_token_uri: "ipfs://Qm......".to_string(),
            collection_uri: "ipfs://Qm......".to_string(),
            seller_fee_bps: 100,
            referral_fee_bps: 1,
            contributors: contributors.clone(),
            start_time: Timestamp::from_seconds(0),
            party_type: PartyType::MaxEdition(1),
            bs721_royalties_code_id: BS721_ROYALTIES_CODE_ID,
            bs721_base_code_id: BS721_BASE_CODE_ID,
        };

        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

        instantiate(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

        assert_eq!(
            res.messages,
            vec![
                SubMsg {
                    msg: WasmMsg::Instantiate {
                        code_id: BS721_BASE_CODE_ID,
                        msg: to_binary(&Bs721BaseInstantiateMsg {
                            name: msg.name.clone(),
                            symbol: msg.symbol.clone(),
                            minter: MOCK_CONTRACT_ADDR.to_string(),
                            uri: Some(msg.collection_uri),
                        })
                        .unwrap(),
                        funds: vec![],
                        admin: None,
                        label: String::from("Launchparty fixed contract"),
                    }
                    .into(),
                    id: INSTANTIATE_TOKEN_REPLY_ID,
                    gas_limit: None,
                    reply_on: ReplyOn::Success,
                },
                SubMsg {
                    msg: WasmMsg::Instantiate {
                        code_id: BS721_ROYALTIES_CODE_ID,
                        msg: to_binary(&Bs721RoyaltiesInstantiateMsg {
                            denom: String::from("ubtsg"),
                            contributors
                        })
                        .unwrap(),
                        label: "Launchparty royalties contract".to_string(),
                        admin: None,
                        funds: vec![],
                    }
                    .into(),
                    id: INSTANTIATE_ROYALTIES_REPLY_ID,
                    gas_limit: None,
                    reply_on: ReplyOn::Success,
                },
            ]
        );

        let instantiate_reply_bs721 = MsgInstantiateContractResponse {
            contract_address: NFT_CONTRACT_ADDR.to_string(),
            data: vec![2u8; 32769],
        };

        let mut encoded_instantiate_reply_bs721 =
            Vec::<u8>::with_capacity(instantiate_reply_bs721.encoded_len());
        instantiate_reply_bs721
            .encode(&mut encoded_instantiate_reply_bs721)
            .unwrap();

        let reply_msg_bs721 = Reply {
            id: INSTANTIATE_TOKEN_REPLY_ID,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![],
                data: Some(encoded_instantiate_reply_bs721.into()),
            }),
        };

        reply(deps.as_mut(), mock_env(), reply_msg_bs721).unwrap();

        let instantiate_reply_royalty = MsgInstantiateContractResponse {
            contract_address: ROYALTIES_CONTRACT_ADDR.to_string(),
            data: vec![2u8; 32769],
        };

        let mut encoded_instantiate_reply_royalty =
            Vec::<u8>::with_capacity(instantiate_reply_royalty.encoded_len());
        instantiate_reply_royalty
            .encode(&mut encoded_instantiate_reply_royalty)
            .unwrap();

        let reply_msg_royalty = Reply {
            id: INSTANTIATE_ROYALTIES_REPLY_ID,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![],
                data: Some(encoded_instantiate_reply_royalty.into()),
            }),
        };

        reply(deps.as_mut(), mock_env(), reply_msg_royalty).unwrap();

        let query_msg = QueryMsg::GetConfig {};
        let res = query(deps.as_ref(), mock_env(), query_msg).unwrap();
        let config: ConfigResponse = from_binary(&res).unwrap();

        assert_eq!(
            config,
            ConfigResponse {
                creator: Addr::unchecked("creator"),
                name: "Launchparty".to_string(),
                symbol: "LP".to_string(),
                base_token_uri: "ipfs://Qm......".to_string(),
                price: coin(1, "ubtsg"),
                max_per_address: Some(1),
                bs721_base: Some(Addr::unchecked(NFT_CONTRACT_ADDR)),
                next_token_id: 1,
                seller_fee_bps: 100,
                referral_fee_bps: 1,
                bs721_royalties: Some(Addr::unchecked(ROYALTIES_CONTRACT_ADDR)),
                start_time: Timestamp::from_nanos(0),
                party_type: PartyType::MaxEdition(1)
            }
        );
    }

    #[test]
    fn mint_single() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            name: "Launchparty".to_string(),
            price: coin(1, "ubtsg"),
            max_per_address: Some(1),
            // creator: Some(String::from("creator")),
            symbol: "LP".to_string(),
            base_token_uri: "ipfs://Qm......".to_string(),
            collection_uri: "ipfs://Qm......".to_string(),
            seller_fee_bps: 100,
            referral_fee_bps: 100,
            contributors: vec![ContributorMsg {
                address: "contributor".to_string(),
                role: "creator".to_string(),
                shares: 100,
            }],
            start_time: Timestamp::from_nanos(0),
            party_type: PartyType::MaxEdition(1),
            bs721_royalties_code_id: 1,
            bs721_base_code_id: 2,
        };

        let info = mock_info("creator", &[coin(1, "ubtsg")]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let instantiate_reply_bs721 = MsgInstantiateContractResponse {
            contract_address: NFT_CONTRACT_ADDR.to_string(),
            data: vec![2u8; 32769],
        };

        let mut encoded_instantiate_reply_bs721 =
            Vec::<u8>::with_capacity(instantiate_reply_bs721.encoded_len());
        instantiate_reply_bs721
            .encode(&mut encoded_instantiate_reply_bs721)
            .unwrap();

        let reply_msg_bs721 = Reply {
            id: INSTANTIATE_TOKEN_REPLY_ID,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![],
                data: Some(encoded_instantiate_reply_bs721.into()),
            }),
        };

        reply(deps.as_mut(), mock_env(), reply_msg_bs721).unwrap();

        let instantiate_reply_royalty = MsgInstantiateContractResponse {
            contract_address: ROYALTIES_CONTRACT_ADDR.to_string(),
            data: vec![2u8; 32769],
        };

        let mut encoded_instantiate_reply_royalty =
            Vec::<u8>::with_capacity(instantiate_reply_royalty.encoded_len());
        instantiate_reply_royalty
            .encode(&mut encoded_instantiate_reply_royalty)
            .unwrap();

        let reply_msg_royalty = Reply {
            id: INSTANTIATE_ROYALTIES_REPLY_ID,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![],
                data: Some(encoded_instantiate_reply_royalty.into()),
            }),
        };

        reply(deps.as_mut(), mock_env(), reply_msg_royalty).unwrap();

        let msg = ExecuteMsg::Mint {
            referral: None,
            amount: 1,
        };
        let info = mock_info(MOCK_CONTRACT_ADDR, &[coin(1, "ubtsg")]);

        let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let mint_msg = Bs721BaseExecuteMsg::<Extension, Empty>::Mint(MintMsg::<Extension> {
            token_id: "1".to_string(),
            extension: None,
            owner: info.sender.to_string(),
            payment_addr: Some(ROYALTIES_CONTRACT_ADDR.to_string()),
            seller_fee_bps: Some(100),
            token_uri: Some("ipfs://Qm....../1".to_string()),
        });

        assert_eq!(
            res.messages[0],
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: NFT_CONTRACT_ADDR.to_string(),
                    funds: vec![],
                    msg: to_binary(&mint_msg).unwrap(),
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );
    }

    #[test]
    fn mint_multiple() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            name: "Launchparty".to_string(),
            price: coin(1, "ubtsg"),
            max_per_address: Some(3),
            // creator: Some(String::from("creator")),
            symbol: "LP".to_string(),
            base_token_uri: "ipfs://Qm......".to_string(),
            collection_uri: "ipfs://Qm......".to_string(),
            seller_fee_bps: 100,
            referral_fee_bps: 100,
            contributors: vec![ContributorMsg {
                address: "contributor".to_string(),
                role: "creator".to_string(),
                shares: 100,
            }],
            start_time: Timestamp::from_nanos(0),
            party_type: PartyType::MaxEdition(3),
            bs721_royalties_code_id: 1,
            bs721_base_code_id: 2,
        };

        let info = mock_info("creator", &[coin(3, "ubtsg")]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let instantiate_reply_bs721 = MsgInstantiateContractResponse {
            contract_address: NFT_CONTRACT_ADDR.to_string(),
            data: vec![2u8; 32769],
        };

        let mut encoded_instantiate_reply_bs721 =
            Vec::<u8>::with_capacity(instantiate_reply_bs721.encoded_len());
        instantiate_reply_bs721
            .encode(&mut encoded_instantiate_reply_bs721)
            .unwrap();

        let reply_msg_bs721 = Reply {
            id: INSTANTIATE_TOKEN_REPLY_ID,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![],
                data: Some(encoded_instantiate_reply_bs721.into()),
            }),
        };

        reply(deps.as_mut(), mock_env(), reply_msg_bs721).unwrap();

        let instantiate_reply_royalty = MsgInstantiateContractResponse {
            contract_address: ROYALTIES_CONTRACT_ADDR.to_string(),
            data: vec![2u8; 32769],
        };

        let mut encoded_instantiate_reply_royalty =
            Vec::<u8>::with_capacity(instantiate_reply_royalty.encoded_len());
        instantiate_reply_royalty
            .encode(&mut encoded_instantiate_reply_royalty)
            .unwrap();

        let reply_msg_royalty = Reply {
            id: INSTANTIATE_ROYALTIES_REPLY_ID,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![],
                data: Some(encoded_instantiate_reply_royalty.into()),
            }),
        };

        reply(deps.as_mut(), mock_env(), reply_msg_royalty).unwrap();

        let msg = ExecuteMsg::Mint {
            referral: None,
            amount: 3,
        };
        let info = mock_info(MOCK_CONTRACT_ADDR, &[coin(3, "ubtsg")]);

        let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let mint_msg = Bs721BaseExecuteMsg::<Extension, Empty>::Mint(MintMsg::<Extension> {
            token_id: "1".to_string(),
            extension: None,
            owner: info.sender.to_string(),
            payment_addr: Some(ROYALTIES_CONTRACT_ADDR.to_string()),
            seller_fee_bps: Some(100),
            token_uri: Some("ipfs://Qm....../1".to_string()),
        });

        assert_eq!(
            res.messages[0],
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: NFT_CONTRACT_ADDR.to_string(),
                    funds: vec![],
                    msg: to_binary(&mint_msg).unwrap(),
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );

        let mint_msg = Bs721BaseExecuteMsg::<Extension, Empty>::Mint(MintMsg::<Extension> {
            token_id: "2".to_string(),
            extension: None,
            owner: info.sender.to_string(),
            payment_addr: Some(ROYALTIES_CONTRACT_ADDR.to_string()),
            seller_fee_bps: Some(100),
            token_uri: Some("ipfs://Qm....../2".to_string()),
        });

        assert_eq!(
            res.messages[1],
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: NFT_CONTRACT_ADDR.to_string(),
                    funds: vec![],
                    msg: to_binary(&mint_msg).unwrap(),
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );

        let mint_msg = Bs721BaseExecuteMsg::<Extension, Empty>::Mint(MintMsg::<Extension> {
            token_id: "3".to_string(),
            extension: None,
            owner: info.sender.to_string(),
            payment_addr: Some(ROYALTIES_CONTRACT_ADDR.to_string()),
            seller_fee_bps: Some(100),
            token_uri: Some("ipfs://Qm....../3".to_string()),
        });

        assert_eq!(
            res.messages[2],
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: NFT_CONTRACT_ADDR.to_string(),
                    funds: vec![],
                    msg: to_binary(&mint_msg).unwrap(),
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );
    }
}
