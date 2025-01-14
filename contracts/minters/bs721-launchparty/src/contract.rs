use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MaxPerAddressResponse, PartyType, QueryMsg};
use crate::state::{Config, EditionMetadata, Trait, ADDRESS_TOKENS, CONFIG};

use bs721_base::{ExecuteMsg as Bs721BaseExecuteMsg, InstantiateMsg as Bs721BaseInstantiateMsg};

use cosmos_sdk_proto::{cosmos::distribution::v1beta1::MsgFundCommunityPool, traits::Message};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, coin, to_json_binary, Addr, Attribute, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut,
    Env, MessageInfo, Reply, ReplyOn, Response, StdError, StdResult, SubMsg, Timestamp, Uint128,
    WasmMsg,
};
use cw2::set_contract_version;

use cw_utils::{may_pay, parse_reply_instantiate_data};

const CONTRACT_NAME: &str = "crates.io:bs721-launchparty";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// ID used to recognize the instantiate token reply in the reply entry point.
const INSTANTIATE_TOKEN_REPLY_ID: u64 = 1;
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

    msg.validate(env.clone())?;

    let start_time = if msg.start_time < env.block.time {
        env.block.time
    } else {
        msg.start_time
    };

    let payment_address = deps.api.addr_validate(&msg.payment_address).map_err(|_| {
        StdError::generic_err(format!(
            "payment address {} is not a valid address",
            msg.payment_address
        ))
    })?;

    let bs721_admin = deps.api.addr_validate(msg.bs721_admin.as_str())?;

    let config = Config {
        creator: info.sender,
        symbol: msg.symbol.clone(),
        name: msg.name.clone(),
        uri: msg.uri.clone(),
        price: msg.price,
        max_per_address: msg.max_per_address,
        bs721_address: None,
        next_token_id: 1, // first token ID is 1
        payment_address,
        seller_fee_bps: msg.seller_fee_bps,
        referral_fee_bps: msg.referral_fee_bps,
        start_time,
        party_type: msg.party_type,
        protocol_fee_bps: msg.protocol_fee_bps,
    };

    CONFIG.save(deps.storage, &config)?;

    // create submessages to instantiate nft
    let sub_msgs: Vec<SubMsg> = vec![SubMsg {
        id: INSTANTIATE_TOKEN_REPLY_ID,
        msg: WasmMsg::Instantiate {
            code_id: msg.bs721_code_id,
            msg: to_json_binary(&Bs721BaseInstantiateMsg {
                name: msg.name.clone(),
                symbol: msg.symbol.clone(),
                minter: env.contract.address.to_string(),
                uri: Some(msg.uri.clone()),
            })?,
            label: "Bitsong Studio Launchparty Contract".to_string(),
            admin: Some(bs721_admin.to_string()),
            funds: vec![],
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

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
            if config.bs721_address.is_some() {
                return Err(ContractError::Bs721BaseAlreadyLinked {});
            }

            config.bs721_address = Addr::unchecked(reply_res.contract_address.clone()).into();

            res = res
                .add_attribute("action", "bs721_base_reply")
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

        let mut attributes: Vec<Trait> = vec![Trait {
            trait_type: "Edition".to_string(),
            value: token_id.to_string(),
            display_type: Some("number".to_string()),
        }];

        if let PartyType::MaxEdition(number) = config.party_type {
            attributes.push(Trait {
                trait_type: "Max Editions".to_string(),
                value: number.to_string(),
                display_type: Some("number".to_string()),
            });
            attributes.push(Trait {
                trait_type: "Edition Type".to_string(),
                value: "Limited Edition".to_string(),
                display_type: None,
            });
        } else {
            attributes.push(Trait {
                trait_type: "Edition Type".to_string(),
                value: "Open Edition".to_string(),
                display_type: None,
            });
        }

        let mint_msg = Bs721BaseExecuteMsg::<EditionMetadata>::Mint {
            owner: info.sender.to_string(),
            token_id: token_id.to_string(),
            token_uri: Some(config.uri.clone()),
            extension: EditionMetadata {
                name: format!("{} #{}", config.name, token_id),
                attributes: Some(attributes),
            },
            payment_addr: Some(config.payment_address.clone().to_string()),
            seller_fee_bps: Some(config.seller_fee_bps),
        };

        let msg = WasmMsg::Execute {
            contract_addr: config.bs721_address.clone().unwrap().to_string(),
            msg: to_json_binary(&mint_msg)?,
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
        let (referral_amount, royalties_amount, protocol_amount) =
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

        if protocol_amount > Uint128::zero() {
            res = res.add_submessage(fund_community_pool_msg(
                env,
                coin(protocol_amount.u128(), accepted_denom.clone()),
            ));
        }

        attributes.push(attr("protocol_fee", protocol_amount.u128().to_string()));

        bank_msgs.push(BankMsg::Send {
            to_address: config.payment_address.clone().to_string(),
            amount: vec![coin(royalties_amount.u128(), accepted_denom.clone())],
        });

        attributes.push(attr("royalties", royalties_amount.u128().to_string()));

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

fn fund_community_pool_msg(env: Env, amount: Coin) -> SubMsg {
    let mut buffer = vec![];

    MsgFundCommunityPool {
        amount: vec![cosmos_sdk_proto::cosmos::base::v1beta1::Coin {
            denom: amount.denom,
            amount: amount.amount.to_string(),
        }],
        depositor: env.contract.address.to_string(),
    }
    .encode(&mut buffer)
    .unwrap();

    SubMsg::new(CosmosMsg::Stargate {
        type_url: "/cosmos.distribution.v1beta1.MsgFundCommunityPool".to_string(),
        value: Binary::from(buffer),
    })
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
) -> StdResult<(Uint128, Uint128, Uint128)> {
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

    let protocol_amount = total_amount
        .checked_mul(Uint128::from(config.protocol_fee_bps))
        .map_err(StdError::overflow)?
        .checked_div(Uint128::new(10_000))
        .map_err(StdError::divide_by_zero)?;

    let royalties_amount = total_amount - referral_amount - protocol_amount;
    if royalties_amount <= Uint128::zero() {
        return Err(StdError::generic_err(
            "royalties amount is zero or negative",
        ));
    }

    Ok((referral_amount, royalties_amount, protocol_amount))
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

    if config.bs721_address.is_none() {
        return Err(ContractError::Bs721NotLinked {});
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
        QueryMsg::GetConfig {} => to_json_binary(&query_config(deps)?),
        QueryMsg::MaxPerAddress { address } => {
            to_json_binary(&query_max_per_address(deps, address)?)
        }
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

fn query_config(deps: Deps) -> StdResult<Config> {
    let config: Config = CONFIG.load(deps.storage)?;

    Ok(config)
}

// -------------------------------------------------------------------------------------------------
// Unit test
// -------------------------------------------------------------------------------------------------
#[cfg(test)]
mod tests {

    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{from_json, to_json_binary, SubMsgResponse, SubMsgResult, Timestamp};
    use prost::Message;

    const NFT_CONTRACT_ADDR: &str = "nftcontract";
    const ROYALTIES_CONTRACT_ADDR: &str = "royaltiescontract";
    const BS721_CODE_ID: u64 = 1;

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
            symbol: String::from(""),
            name: String::from(""),
            uri: String::from(""),
            price: coin(1, "ubtsg"),
            max_per_address: None,
            next_token_id: 1,
            payment_address: Addr::unchecked("payment_address"),
            seller_fee_bps: 1_000,
            referral_fee_bps: 1_000,
            protocol_fee_bps: 1_000,
            start_time: Timestamp::from_seconds(1),
            party_type: PartyType::MaxEdition(2),
            bs721_address: Some(Addr::unchecked("contract1")),
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
            config.bs721_address = None;
            let resp = before_mint_checks(&env, &config, 1).unwrap_err();
            assert_eq!(
                resp,
                ContractError::Bs721NotLinked {},
                "expected to fail since cw721 base contract not linked"
            );
            config.bs721_address = Some(Addr::unchecked("contract1"));
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
            symbol: String::from(""),
            name: String::from(""),
            uri: String::from(""),
            price: coin(1, "ubtsg"),
            max_per_address: None,
            next_token_id: 1,
            seller_fee_bps: 1_000,
            referral_fee_bps: 1_000,
            protocol_fee_bps: 1_000,
            start_time: Timestamp::from_seconds(1),
            party_type: PartyType::MaxEdition(2),
            bs721_address: Some(Addr::unchecked("contract1")),
            payment_address: Addr::unchecked("contract2"),
        };

        {
            let (referral_amt, royalties_amt, protocol_amt) =
                compute_referral_and_royalties_amounts(&config, &None, Uint128::new(1_000))
                    .unwrap();
            assert_eq!(
                Uint128::zero(),
                referral_amt,
                "expected zero referral amount since no referral address"
            );
            assert_eq!(
                Uint128::new(900),
                royalties_amt,
                "expected royalties amount equal to total amount - protocol_fee"
            );
            assert_eq!(Uint128::new(100), protocol_amt, "expected protocol fee")
        }

        {
            let (referral_amt, royalties_amt, protocol_amt) =
                compute_referral_and_royalties_amounts(
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
                Uint128::new(800),
                royalties_amt,
                "expected 80% as royalties amount"
            );
            assert_eq!(Uint128::new(100), protocol_amt, "expected protocol fee")
        }

        {
            // must get an error
            let result = compute_referral_and_royalties_amounts(
                &config,
                &Some(Addr::unchecked("referrral".to_string())),
                Uint128::zero(),
            );

            assert!(result.is_err());
            match result {
                Err(StdError::GenericErr { msg }) => {
                    assert_eq!(msg, "royalties amount is zero or negative");
                }
                _ => panic!("Unexpected error"),
            }
        }

        {
            let (referral_amt, royalties_amt, protocol_amt) =
                compute_referral_and_royalties_amounts(
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
            assert_eq!(Uint128::new(0), protocol_amt, "expected zero protocol fee")
        }

        {
            let (referral_amt, royalties_amt, protocol_amt) =
                compute_referral_and_royalties_amounts(
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
            assert_eq!(Uint128::new(0), protocol_amt, "expected zero protocol fee")
        }

        {
            let (referral_amt, royalties_amt, protocol_amt) =
                compute_referral_and_royalties_amounts(
                    &config,
                    &Some(Addr::unchecked("referrral".to_string())),
                    Uint128::new(10),
                )
                .unwrap();
            assert_eq!(Uint128::new(1), referral_amt, "expected 1 since 10% of 10");
            assert_eq!(
                Uint128::new(8),
                royalties_amt,
                "expected 8 since royalties is 10 minus referral amount minus protocol fee"
            );
            assert_eq!(Uint128::new(1), protocol_amt, "expected 1 protocol fee")
        }
    }

    #[test]
    fn initialization_fails() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let msg = InstantiateMsg {
            price: coin(1, "ubtsg"),
            max_per_address: Some(1),
            // creator: Some(String::from("creator")),
            payment_address: String::from(ROYALTIES_CONTRACT_ADDR),
            symbol: String::from(""),
            name: String::from(""),
            uri: String::from(""),
            seller_fee_bps: 100,
            referral_fee_bps: 1,
            protocol_fee_bps: 3,
            start_time: env.block.time,
            party_type: PartyType::MaxEdition(1),
            bs721_code_id: BS721_CODE_ID,
            bs721_admin: String::from("bs721_admin"),
        };

        let info = mock_info("creator", &[]);
        instantiate(deps.as_mut(), env, info, msg).unwrap();
    }

    #[test]
    fn initialization() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let msg = InstantiateMsg {
            price: coin(1, "ubtsg"),
            max_per_address: Some(1),
            // creator: Some(String::from("creator")),
            symbol: String::from(""),
            name: String::from(""),
            uri: String::from(""),
            seller_fee_bps: 100,
            referral_fee_bps: 1,
            protocol_fee_bps: 3,
            start_time: env.block.time,
            party_type: PartyType::MaxEdition(1),
            bs721_code_id: BS721_CODE_ID,
            payment_address: String::from(ROYALTIES_CONTRACT_ADDR),
            bs721_admin: String::from("bs721_admin"),
        };

        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

        instantiate(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();

        assert_eq!(
            res.messages,
            vec![SubMsg {
                msg: WasmMsg::Instantiate {
                    code_id: BS721_CODE_ID,
                    msg: to_json_binary(&Bs721BaseInstantiateMsg {
                        name: msg.name.clone(),
                        symbol: msg.symbol.clone(),
                        minter: MOCK_CONTRACT_ADDR.to_string(),
                        uri: Some(String::from("")),
                    })
                    .unwrap(),
                    funds: vec![],
                    admin: Some(String::from("bs721_admin")),
                    label: String::from("Bitsong Studio Launchparty Contract"),
                }
                .into(),
                id: INSTANTIATE_TOKEN_REPLY_ID,
                gas_limit: None,
                reply_on: ReplyOn::Success,
            }]
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

        reply(deps.as_mut(), env.clone(), reply_msg_bs721).unwrap();

        let query_msg = QueryMsg::GetConfig {};
        let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
        let config: Config = from_json(&res).unwrap();

        assert_eq!(
            config,
            Config {
                creator: Addr::unchecked("creator"),
                symbol: String::from(""),
                name: String::from(""),
                uri: String::from(""),
                price: coin(1, "ubtsg"),
                max_per_address: Some(1),
                next_token_id: 1,
                seller_fee_bps: 100,
                referral_fee_bps: 1,
                protocol_fee_bps: 3,
                start_time: env.block.time,
                party_type: PartyType::MaxEdition(1),
                bs721_address: Some(Addr::unchecked(NFT_CONTRACT_ADDR)),
                payment_address: Addr::unchecked(ROYALTIES_CONTRACT_ADDR),
            }
        );
    }

    #[test]
    fn mint_single() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let msg = InstantiateMsg {
            price: coin(1, "ubtsg"),
            max_per_address: Some(1),
            symbol: "LP".to_string(),
            name: "Launchparty".to_string(),
            uri: String::from(""),
            seller_fee_bps: 100,
            referral_fee_bps: 100,
            start_time: env.block.time,
            party_type: PartyType::MaxEdition(1),
            bs721_code_id: 2,
            protocol_fee_bps: 3,
            payment_address: String::from(ROYALTIES_CONTRACT_ADDR),
            bs721_admin: String::from("bs721_admin"),
        };

        let info = mock_info("creator", &[coin(1, "ubtsg")]);
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

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

        reply(deps.as_mut(), env.clone(), reply_msg_bs721).unwrap();

        let msg = ExecuteMsg::Mint {
            referral: None,
            amount: 1,
        };
        let info = mock_info(MOCK_CONTRACT_ADDR, &[coin(1, "ubtsg")]);

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let mint_msg = Bs721BaseExecuteMsg::<EditionMetadata>::Mint {
            token_id: "1".to_string(),
            extension: EditionMetadata {
                name: format!("{} #{}", "Launchparty".to_string(), "1".to_string()),
                attributes: Some(vec![
                    Trait {
                        trait_type: "Edition".to_string(),
                        value: "1".to_string(),
                        display_type: Some("number".to_string()),
                    },
                    Trait {
                        trait_type: "Max Editions".to_string(),
                        value: "1".to_string(),
                        display_type: Some("number".to_string()),
                    },
                    Trait {
                        trait_type: "Edition Type".to_string(),
                        value: "Limited Edition".to_string(),
                        display_type: None,
                    },
                ]),
            },
            owner: info.sender.to_string(),
            payment_addr: Some(ROYALTIES_CONTRACT_ADDR.to_string()),
            seller_fee_bps: Some(100),
            token_uri: Some(String::from("")),
        };

        assert_eq!(
            res.messages[0],
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: NFT_CONTRACT_ADDR.to_string(),
                    funds: vec![],
                    msg: to_json_binary(&mint_msg).unwrap(),
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
        let env = mock_env();
        let msg = InstantiateMsg {
            price: coin(1, "ubtsg"),
            max_per_address: Some(3),
            symbol: "LP".to_string(),
            name: "Launchparty".to_string(),
            uri: String::from(""),
            seller_fee_bps: 100,
            referral_fee_bps: 100,
            start_time: env.block.time,
            party_type: PartyType::MaxEdition(3),
            bs721_code_id: 2,
            protocol_fee_bps: 3,
            payment_address: String::from(ROYALTIES_CONTRACT_ADDR),
            bs721_admin: String::from("bs721_admin"),
        };

        let info = mock_info("creator", &[coin(3, "ubtsg")]);
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

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

        reply(deps.as_mut(), env.clone(), reply_msg_bs721).unwrap();

        let msg = ExecuteMsg::Mint {
            referral: None,
            amount: 3,
        };
        let info = mock_info(MOCK_CONTRACT_ADDR, &[coin(3, "ubtsg")]);

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let mint_msg = Bs721BaseExecuteMsg::<EditionMetadata>::Mint {
            token_id: "1".to_string(),
            extension: EditionMetadata {
                name: format!("{} #{}", "Launchparty".to_string(), "1".to_string()),
                attributes: Some(vec![
                    Trait {
                        trait_type: "Edition".to_string(),
                        value: "1".to_string(),
                        display_type: Some("number".to_string()),
                    },
                    Trait {
                        trait_type: "Max Editions".to_string(),
                        value: "3".to_string(),
                        display_type: Some("number".to_string()),
                    },
                    Trait {
                        trait_type: "Edition Type".to_string(),
                        value: "Limited Edition".to_string(),
                        display_type: None,
                    },
                ]),
            },
            owner: info.sender.to_string(),
            payment_addr: Some(ROYALTIES_CONTRACT_ADDR.to_string()),
            seller_fee_bps: Some(100),
            token_uri: Some(String::from("")),
        };

        assert_eq!(
            res.messages[0],
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: NFT_CONTRACT_ADDR.to_string(),
                    funds: vec![],
                    msg: to_json_binary(&mint_msg).unwrap(),
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );

        let mint_msg = Bs721BaseExecuteMsg::<EditionMetadata>::Mint {
            token_id: "2".to_string(),
            extension: EditionMetadata {
                name: format!("{} #{}", "Launchparty".to_string(), "2".to_string()),
                attributes: Some(vec![
                    Trait {
                        trait_type: "Edition".to_string(),
                        value: "2".to_string(),
                        display_type: Some("number".to_string()),
                    },
                    Trait {
                        trait_type: "Max Editions".to_string(),
                        value: "3".to_string(),
                        display_type: Some("number".to_string()),
                    },
                    Trait {
                        trait_type: "Edition Type".to_string(),
                        value: "Limited Edition".to_string(),
                        display_type: None,
                    },
                ]),
            },
            owner: info.sender.to_string(),
            payment_addr: Some(ROYALTIES_CONTRACT_ADDR.to_string()),
            seller_fee_bps: Some(100),
            token_uri: Some(String::from("")),
        };

        assert_eq!(
            res.messages[1],
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: NFT_CONTRACT_ADDR.to_string(),
                    funds: vec![],
                    msg: to_json_binary(&mint_msg).unwrap(),
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );

        let mint_msg = Bs721BaseExecuteMsg::<EditionMetadata>::Mint {
            token_id: "3".to_string(),
            extension: EditionMetadata {
                name: format!("{} #{}", "Launchparty".to_string(), "3".to_string()),
                attributes: Some(vec![
                    Trait {
                        trait_type: "Edition".to_string(),
                        value: "3".to_string(),
                        display_type: Some("number".to_string()),
                    },
                    Trait {
                        trait_type: "Max Editions".to_string(),
                        value: "3".to_string(),
                        display_type: Some("number".to_string()),
                    },
                    Trait {
                        trait_type: "Edition Type".to_string(),
                        value: "Limited Edition".to_string(),
                        display_type: None,
                    },
                ]),
            },
            owner: info.sender.to_string(),
            payment_addr: Some(ROYALTIES_CONTRACT_ADDR.to_string()),
            seller_fee_bps: Some(100),
            token_uri: Some(String::from("")),
        };

        assert_eq!(
            res.messages[2],
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: NFT_CONTRACT_ADDR.to_string(),
                    funds: vec![],
                    msg: to_json_binary(&mint_msg).unwrap(),
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );
    }
}
