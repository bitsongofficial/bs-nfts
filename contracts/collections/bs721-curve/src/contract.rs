use std::ops::Add;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MaxPerAddressResponse, PriceResponse, QueryMsg};
use crate::state::{Config, EditionMetadata, Trait, ADDRESS_TOKENS, CONFIG};

use cosmos_sdk_proto::{cosmos::distribution::v1beta1::MsgFundCommunityPool, traits::Message};

use bs721::{Bs721QueryMsg, NumTokensResponse};
use bs721_base::{ExecuteMsg as Bs721BaseExecuteMsg, InstantiateMsg as Bs721BaseInstantiateMsg};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, coin, to_json_binary, Addr, Attribute, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps,
    DepsMut, Empty, Env, MessageInfo, QuerierWrapper, QueryRequest, Reply, ReplyOn, Response,
    StdError, StdResult, Storage, SubMsg, Uint128, WasmMsg, WasmQuery,
};
use cw2::set_contract_version;

use cw_utils::{must_pay, nonpayable, parse_reply_instantiate_data};

const CONTRACT_NAME: &str = "crates.io:bs721-curve";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// ID used to recognize the instantiate token reply in the reply entry point.
const INSTANTIATE_TOKEN_REPLY_ID: u64 = 1;

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

    let payment_address = deps.api.addr_validate(msg.payment_address.as_str())?;
    let bs721_admin = deps.api.addr_validate(msg.bs721_admin.as_str())?;

    let config = Config {
        creator: info.sender,
        symbol: msg.symbol.clone(),
        name: msg.name.clone(),
        uri: msg.uri.clone(),
        payment_denom: msg.payment_denom.clone(),
        max_per_address: msg.max_per_address,
        bs721_address: None,
        next_token_id: 1, // first token ID is 1
        seller_fee_bps: msg.seller_fee_bps,
        referral_fee_bps: msg.referral_fee_bps,
        protocol_fee_bps: msg.protocol_fee_bps,
        payment_address,
        start_time,
        max_edition: msg.max_edition,
        ratio: msg.ratio,
    };

    CONFIG.save(deps.storage, &config)?;

    // create submessages to instantiate token and royalties contracts
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
            label: "Bitsong Studio Curve Contract".to_string(),
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
        ExecuteMsg::Burn {
            token_ids,
            referral,
            min_out_amount,
        } => {
            // check if referral address is valid
            let referral = referral
                .map(|address| deps.api.addr_validate(address.as_str()))
                .transpose()?;
            execute_burn(deps, env, info, token_ids, min_out_amount, referral)
        }
    }
}

// Sum of squares of first n natural numbers
// n * (n + 1) * (2 * n + 1) / 6;
fn sum_of_squares(n: Uint128) -> Uint128 {
    if n.is_zero() {
        Uint128::zero()
    } else {
        n * (n + Uint128::one()) * (Uint128::new(2) * n + Uint128::one()) / Uint128::new(6)
    }
}

fn compute_base_price(storage: &dyn Storage, supply: Uint128, amount: Uint128) -> Uint128 {
    let config: Config = CONFIG.load(storage).unwrap();

    let sum1 = sum_of_squares(supply);
    let sum2 = sum_of_squares(supply.add(amount));

    (sum2 - sum1) * Uint128::new(1_000_000) / Uint128::from(config.ratio)
}

fn buy_price(storage: &dyn Storage, supply: Uint128, amount: Uint128) -> PriceResponse {
    let config = CONFIG.load(storage).unwrap();

    let base_price = Decimal::from_ratio(compute_base_price(storage, supply, amount), 1u32);

    let royalties_fee = Decimal::from_ratio(config.seller_fee_bps, 10000u32);
    let royalties_price = base_price * royalties_fee;

    let referral_fee = Decimal::from_ratio(config.referral_fee_bps, 10000u32);
    let referral_price = royalties_price * referral_fee;

    let protocol_fee = Decimal::from_ratio(config.protocol_fee_bps, 10000u32);
    let protocol_price = base_price * protocol_fee;

    let total_price =
        base_price + (royalties_price - referral_price) + referral_price + protocol_price;

    PriceResponse {
        base_price: base_price.to_uint_floor(),
        royalties: (royalties_price - referral_price).to_uint_floor(),
        referral: referral_price.to_uint_floor(),
        protocol_fee: protocol_price.to_uint_floor(),
        total_price: total_price.to_uint_floor(),
    }
}

fn sell_price(storage: &dyn Storage, supply: Uint128, amount: Uint128) -> PriceResponse {
    if amount > supply {
        return PriceResponse {
            base_price: Uint128::zero(),
            royalties: Uint128::zero(),
            referral: Uint128::zero(),
            protocol_fee: Uint128::zero(),
            total_price: Uint128::zero(),
        };
    }

    let config = CONFIG.load(storage).unwrap();

    let base_price = Decimal::from_ratio(
        compute_base_price(storage, supply.checked_sub(amount).unwrap(), amount),
        1u32,
    );

    let royalties_fee = Decimal::from_ratio(config.seller_fee_bps, 10000u32);
    let royalties_price = base_price * royalties_fee;

    let referral_fee = Decimal::from_ratio(config.referral_fee_bps, 10000u32);
    let referral_price = royalties_price * referral_fee;

    let protocol_fee = Decimal::from_ratio(config.protocol_fee_bps, 10000u32);
    let protocol_price = base_price * protocol_fee;

    let total_price =
        base_price - (royalties_price - referral_price) - referral_price - protocol_price;

    PriceResponse {
        base_price: base_price.to_uint_floor(),
        royalties: (royalties_price - referral_price).to_uint_floor(),
        referral: referral_price.to_uint_floor(),
        protocol_fee: protocol_price.to_uint_floor(),
        total_price: total_price.to_uint_floor(),
    }
}

fn query_supply(querier: QuerierWrapper, storage: &dyn Storage) -> Uint128 {
    let config: Config = CONFIG.load(storage).unwrap();

    let query_msg = Bs721QueryMsg::NumTokens {};

    let query_response: NumTokensResponse = querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.bs721_address.unwrap().into_string(),
            msg: to_json_binary(&query_msg).unwrap(),
        }))
        .unwrap();

    Uint128::from(query_response.count)
}

fn execute_burn(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_ids: Vec<u32>,
    min_out_amount: u128,
    referral: Option<Addr>,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    let config: Config = CONFIG.load(deps.storage)?;
    let payment_denom = config.payment_denom.clone();

    let amount = token_ids.clone().len() as u32;
    if amount == 0 {
        return Err(ContractError::Std(StdError::generic_err(
            "no token ids provided",
        )));
    }

    let mut res = Response::new();
    for token_id in token_ids.clone() {
        let burn_msg = WasmMsg::Execute {
            contract_addr: config.bs721_address.clone().unwrap().to_string(),
            msg: to_json_binary(&Bs721BaseExecuteMsg::<Empty>::Burn {
                token_id: token_id.to_string(),
            })?,
            funds: vec![],
        };

        res = res.add_message(burn_msg);
    }

    let supply = query_supply(deps.querier, deps.storage);
    let price = sell_price(deps.storage, supply, amount.into());

    if min_out_amount > price.total_price.u128() {
        return Err(ContractError::MinOutAmount {
            min_out_amount,
            amount: price.total_price.u128(),
        });
    }

    let mut bank_msgs: Vec<BankMsg> = vec![];
    let mut attributes: Vec<Attribute> = vec![];

    let mut royalties_sum = price.royalties;

    // Pay referral
    if !referral.is_none() {
        if !price.referral.is_zero() {
            bank_msgs.push(BankMsg::Send {
                to_address: referral.clone().unwrap().to_string(),
                amount: vec![coin(price.referral.u128(), payment_denom.clone())],
            });

            attributes.push(attr("referral", referral.unwrap().to_string()));
            attributes.push(attr("referral_amount", price.referral.u128().to_string()));
        }
    } else {
        royalties_sum = price.royalties + price.referral;
    }

    // Pay royalties
    bank_msgs.push(BankMsg::Send {
        to_address: config.payment_address.clone().to_string(),
        amount: vec![coin(royalties_sum.u128(), payment_denom.clone())],
    });

    attributes.push(attr("royalties", royalties_sum.u128().to_string()));

    attributes.push(attr(
        "royalties_recipient",
        config.payment_address.clone().to_string(),
    ));

    // Protocol Fee
    res = res.add_submessage(fund_community_pool_msg(
        env,
        coin(price.protocol_fee.u128(), payment_denom.clone()),
    ));

    attributes.push(attr("protocol_fee", price.protocol_fee.u128().to_string()));

    // Pay seller
    bank_msgs.push(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![coin(price.total_price.u128(), payment_denom.clone())],
    });

    res = res.add_messages(bank_msgs).add_attributes(attributes);

    // decrease the number of tokens minted by the sender
    let already_minted = (ADDRESS_TOKENS.key(&info.sender).may_load(deps.storage)?).unwrap_or(0);
    let new_total_mint = already_minted.checked_sub(amount).unwrap_or(0);
    ADDRESS_TOKENS.save(deps.storage, &info.sender, &new_total_mint)?;

    let token_ids_str: Vec<String> = token_ids.iter().map(|&n| n.to_string()).collect();

    Ok(res
        .add_attribute("action", "burn_bs721_curve_nft")
        .add_attribute("token_ids", token_ids_str.join(","))
        .add_attribute("price", price.total_price)
        .add_attribute("denom", config.payment_denom)
        .add_attribute("creator", config.creator.to_string())
        .add_attribute("burner", info.sender.to_string()))
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

fn execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: u32,
    referral: Option<Addr>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    let payment_denom = config.payment_denom.clone();

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

    let sent_amount = must_pay(&info, &payment_denom)?;
    let supply = query_supply(deps.querier, deps.storage);
    let price = buy_price(deps.storage, supply, amount.into());

    if sent_amount < price.total_price {
        return Err(ContractError::InvalidPaymentAmount(
            sent_amount,
            price.total_price,
        ));
    }

    let mut res = Response::new();
    let mut token_ids: Vec<u32> = vec![];

    // create minting message
    for _ in 0..amount {
        let token_id = config.next_token_id;

        let mut attributes: Vec<Trait> = vec![Trait {
            trait_type: "Edition".to_string(),
            value: token_id.to_string(),
            display_type: Some("number".to_string()),
        }];

        if let Some(max_edition) = config.max_edition {
            attributes.push(Trait {
                trait_type: "Max Editions".to_string(),
                value: max_edition.to_string(),
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

        res = res.add_message(msg);

        token_ids.push(token_id);

        config.next_token_id += 1;
        CONFIG.save(deps.storage, &config)?;
    }

    let mut bank_msgs: Vec<BankMsg> = vec![];
    let mut attributes: Vec<Attribute> = vec![];

    let mut royalties_sum = price.royalties;

    // Pay referral
    if !referral.is_none() {
        if !price.referral.is_zero() {
            bank_msgs.push(BankMsg::Send {
                to_address: referral.clone().unwrap().to_string(),
                amount: vec![coin(price.referral.u128(), payment_denom.clone())],
            });

            attributes.push(attr("referral", referral.unwrap().to_string()));
            attributes.push(attr("referral_amount", price.referral.u128().to_string()));
        }
    } else {
        royalties_sum = price.royalties + price.referral;
    }

    // Pay royalties
    bank_msgs.push(BankMsg::Send {
        to_address: config.payment_address.clone().to_string(),
        amount: vec![coin(royalties_sum.u128(), payment_denom.clone())],
    });

    attributes.push(attr("royalties", royalties_sum.u128().to_string()));

    attributes.push(attr(
        "royalties_recipient",
        config.payment_address.clone().to_string(),
    ));

    // Protocol fee
    res = res.add_submessage(fund_community_pool_msg(
        env,
        coin(price.protocol_fee.u128(), payment_denom.clone()),
    ));

    attributes.push(attr("protocol_fee", price.protocol_fee.u128().to_string()));

    // Refund if needed
    let refund_amount = sent_amount - price.total_price;
    if !refund_amount.is_zero() {
        bank_msgs.push(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![coin(refund_amount.u128(), payment_denom.clone())],
        });

        attributes.push(attr("refund", refund_amount.u128().to_string()));
    }

    res = res.add_messages(bank_msgs).add_attributes(attributes);

    CONFIG.save(deps.storage, &config)?;
    ADDRESS_TOKENS.save(deps.storage, &info.sender, &new_total_mint)?;

    let token_ids_str: Vec<String> = token_ids.iter().map(|&n| n.to_string()).collect();

    Ok(res
        .add_attribute("action", "mint_bs721_curve_nft")
        .add_attribute("token_ids", token_ids_str.join(","))
        .add_attribute("price", price.total_price)
        .add_attribute("denom", config.payment_denom)
        .add_attribute("creator", config.creator.to_string())
        .add_attribute("recipient", info.sender.to_string()))
}

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

    let max_editions = config.max_edition.unwrap_or(0);
    if max_editions > 0 {
        if (config.next_token_id - 1) + edition_to_mint > max_editions {
            return Err(ContractError::SoldOut {});
        }
    }

    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&query_config(deps)?),
        QueryMsg::MaxPerAddress { address } => {
            to_json_binary(&query_max_per_address(deps, address)?)
        }
        QueryMsg::BuyPrice { amount } => {
            to_json_binary(&query_buy_price(deps, Uint128::new(amount))?)
        }
        QueryMsg::SellPrice { amount } => {
            to_json_binary(&query_sell_price(deps, Uint128::new(amount))?)
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

fn query_buy_price(deps: Deps, amount: Uint128) -> StdResult<PriceResponse> {
    let supply = query_supply(deps.querier, deps.storage);

    Ok(buy_price(deps.storage, supply, amount))
}

fn query_sell_price(deps: Deps, amount: Uint128) -> StdResult<PriceResponse> {
    let supply = query_supply(deps.querier, deps.storage);

    Ok(sell_price(deps.storage, supply, amount))
}

// -------------------------------------------------------------------------------------------------
// Unit tests
// -------------------------------------------------------------------------------------------------
#[cfg(test)]
mod test {
    use cosmwasm_std::{testing::mock_dependencies, Timestamp};

    use super::*;

    #[test]
    fn sum_of_squares_zero() {
        assert_eq!(sum_of_squares(Uint128::zero()), Uint128::zero());
    }

    #[test]
    fn sum_of_squares_small_positive() {
        let n = Uint128::new(1);
        assert_eq!(sum_of_squares(n), Uint128::new(1));
    }

    #[test]
    fn test_sum_of_squares_large_positive() {
        let n = Uint128::new(5_541_000_000_000);
        assert_eq!(
            sum_of_squares(n),
            Uint128::new(56_707_851_807_015_351_340_500_000_923_500_000_000)
        );
    }

    #[test]
    fn compute_base_price_zero_supply_amount() {
        let mut deps = mock_dependencies();

        let config = Config {
            creator: Addr::unchecked("creator"),
            symbol: "TEST".to_string(),
            payment_denom: "ubtsg".to_string(),
            max_per_address: None,
            bs721_address: None,
            name: "Test".to_string(),
            uri: "ipfs://Qm......".to_string(),
            next_token_id: 1,
            seller_fee_bps: 0,
            referral_fee_bps: 0,
            protocol_fee_bps: 0,
            payment_address: Addr::unchecked("payment_address"),
            start_time: Timestamp::from_seconds(0),
            max_edition: None,
            ratio: 1,
        };

        CONFIG.save(deps.as_mut().storage, &config).unwrap();

        assert_eq!(
            compute_base_price(deps.as_ref().storage, Uint128::zero(), Uint128::zero()),
            Uint128::zero()
        );
    }

    #[test]
    fn compute_base_price_non_zero_supply_zero_amount() {
        let mut deps = mock_dependencies();

        let config = Config {
            creator: Addr::unchecked("creator"),
            symbol: "TEST".to_string(),
            payment_denom: "ubtsg".to_string(),
            max_per_address: None,
            bs721_address: None,
            name: "Test".to_string(),
            uri: "ipfs://Qm......".to_string(),
            next_token_id: 1,
            seller_fee_bps: 0,
            referral_fee_bps: 0,
            protocol_fee_bps: 0,
            payment_address: Addr::unchecked("payment_address"),
            start_time: Timestamp::from_seconds(0),
            max_edition: None,
            ratio: 1,
        };

        CONFIG.save(deps.as_mut().storage, &config).unwrap();

        assert_eq!(
            compute_base_price(deps.as_ref().storage, Uint128::new(1), Uint128::zero()),
            Uint128::zero()
        );
    }

    #[test]
    fn test_buy_price() {
        let mut deps = mock_dependencies();

        let config = Config {
            creator: Addr::unchecked("creator"),
            symbol: "TEST".to_string(),
            payment_denom: "ubtsg".to_string(),
            max_per_address: None,
            bs721_address: None,
            name: "Test".to_string(),
            uri: "ipfs://Qm......".to_string(),
            next_token_id: 1,
            seller_fee_bps: 100,
            referral_fee_bps: 0,
            protocol_fee_bps: 30,
            payment_address: Addr::unchecked("payment_address"),
            start_time: Timestamp::from_seconds(0),
            max_edition: None,
            ratio: 1,
        };

        CONFIG.save(deps.as_mut().storage, &config).unwrap();

        let supply = Uint128::new(0);
        let amount = Uint128::new(1);
        let price_response = buy_price(deps.as_ref().storage, supply, amount);

        assert_eq!(
            price_response.base_price,
            Decimal::from_ratio(1000000u128, 1u32).to_uint_floor()
        );
        assert_eq!(
            price_response.royalties,
            Decimal::from_ratio(10000u128, 1u32).to_uint_floor()
        );
        assert_eq!(
            price_response.referral,
            Decimal::from_ratio(0u128, 1u32).to_uint_floor()
        );
        assert_eq!(
            price_response.protocol_fee,
            Decimal::from_ratio(3000u128, 1u32).to_uint_floor()
        );
        assert_eq!(
            price_response.total_price,
            Decimal::from_ratio(1013000u128, 1u32).to_uint_floor()
        );
    }

    #[test]
    fn test_sell_price() {
        let mut deps = mock_dependencies();

        let config = Config {
            creator: Addr::unchecked("creator"),
            symbol: "TEST".to_string(),
            payment_denom: "ubtsg".to_string(),
            max_per_address: None,
            bs721_address: None,
            name: "Test".to_string(),
            uri: "ipfs://Qm......".to_string(),
            next_token_id: 1,
            seller_fee_bps: 100,
            referral_fee_bps: 0,
            protocol_fee_bps: 30,
            payment_address: Addr::unchecked("payment_address"),
            start_time: Timestamp::from_seconds(0),
            max_edition: None,
            ratio: 1,
        };

        CONFIG.save(deps.as_mut().storage, &config).unwrap();

        let supply = Uint128::new(1);
        let amount = Uint128::new(1);
        let price_response = sell_price(deps.as_ref().storage, supply, amount);

        assert_eq!(
            price_response.base_price,
            Decimal::from_ratio(1000000u128, 1u32).to_uint_floor()
        );
        assert_eq!(
            price_response.royalties,
            Decimal::from_ratio(10000u128, 1u32).to_uint_floor()
        );
        assert_eq!(
            price_response.referral,
            Decimal::from_ratio(0u128, 1u32).to_uint_floor()
        );
        assert_eq!(
            price_response.protocol_fee,
            Decimal::from_ratio(3000u128, 1u32).to_uint_floor()
        );
        assert_eq!(
            price_response.total_price,
            Decimal::from_ratio(987000u128, 1u32).to_uint_floor()
        );
    }

    #[test]
    fn compute_base_price_with_values() {
        let mut deps = mock_dependencies();

        // ratio, supply, amount, expected
        let test_cases = [
            (1, 1, 1, 4_000_000),
            (10, 1, 1, 400_000),
            (100, 1, 1, 40_000),
            (1000, 1, 1, 4_000),
            (10000, 1, 1, 400),
            (100000, 1, 1, 40),
            (1000000, 1, 1, 4),
            (10000000, 1, 1, 0),
            (1, 0, 0, 0),
            (1, 10, 5, 855_000_000),
            (1, 100, 50, 797_925_000_000),
            (10, 10, 10, 248_500_000),
            (100, 20, 10, 65_850_000),
            (1000, 30, 15, 21_940_000),
            (10000, 40, 20, 5_167_000),
            (100000, 50, 25, 1_005_250),
            (1, 2, 2, 25_000_000),
            (1, 3, 3, 77_000_000),
            (1, 4, 4, 174_000_000),
            (1, 5, 5, 330_000_000),
            (1, 6, 6, 559_000_000),
            (1, 7, 7, 875_000_000),
            (1, 8, 8, 1_292_000_000),
            (1, 9, 9, 1_824_000_000),
            (1, 10, 10, 2_485_000_000),
            (1, 11, 11, 3_289_000_000),
            (1, 12, 12, 4_250_000_000),
            (1, 13, 13, 5_382_000_000),
            (1, 14, 14, 6_699_000_000),
            (1, 15, 15, 8_215_000_000),
        ];

        for (ratio, supply, amount, expected) in test_cases {
            let config = Config {
                creator: Addr::unchecked("creator"),
                symbol: "TEST".to_string(),
                payment_denom: "ubtsg".to_string(),
                max_per_address: None,
                bs721_address: None,
                name: "Test".to_string(),
                uri: "ipfs://Qm......".to_string(),
                next_token_id: 1,
                seller_fee_bps: 0,
                referral_fee_bps: 0,
                protocol_fee_bps: 0,
                payment_address: Addr::unchecked("payment_address"),
                start_time: Timestamp::from_seconds(0),
                max_edition: None,
                ratio,
            };

            CONFIG.save(deps.as_mut().storage, &config).unwrap();

            assert_eq!(
                compute_base_price(
                    deps.as_ref().storage,
                    Uint128::new(supply),
                    Uint128::new(amount)
                ),
                Uint128::new(expected)
            );
        }
    }
}
