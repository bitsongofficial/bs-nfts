#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Addr, BankMsg, Binary, Coin, Decimal, Deps, DepsMut, Env, MessageInfo, Order,
    Response, StdResult, Storage, Uint128,
};
use cw2::set_contract_version;
use cw_storage_plus::Bound;
use cw_utils::maybe_addr;

use crate::msg::ExecuteMsg;
use crate::state::{DENOM, WITHDRAWABLE_AMOUNT};
use crate::{
    msg::{ContributorListResponse, ContributorResponse, InstantiateMsg, QueryMsg},
    state::{Contributor, CONTRIBUTORS, TOTAL_SHARES},
    ContractError,
};

// version and name info for migration
const CONTRACT_NAME: &str = "crates.io:bs721-royalties";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    mut msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let total_shares = msg.validate_and_compute_total_shares()?;

    for contributor in msg.contributors {
        let contributor_addr = deps.api.addr_validate(&contributor.address)?;

        compute_shares_and_store(
            deps.storage,
            contributor_addr,
            contributor.shares,
            contributor.role,
            total_shares,
        )?;
    }

    WITHDRAWABLE_AMOUNT.save(deps.storage, &Uint128::zero())?;
    TOTAL_SHARES.save(deps.storage, &total_shares)?;
    DENOM.save(deps.storage, &msg.denom)?;

    Ok(Response::default())
}

/// Helper function to store a contributor after computing their percentage shares. Returns the percentage
/// shares associated t `contributor_addr`.
pub fn compute_shares_and_store(
    store: &mut dyn Storage,
    contributor_addr: Addr,
    initial_shares: u32,
    role: String,
    total_shares: Uint128,
) -> Result<Decimal, ContractError> {
    let percentage_shares = Decimal::from_ratio(Uint128::from(initial_shares), total_shares);

    let new_contributor = Contributor {
        role,
        initial_shares,
        percentage_shares,
        withdrawable_amount: Uint128::zero(),
    };

    CONTRIBUTORS.save(store, &contributor_addr, &new_contributor)?;

    Ok(percentage_shares)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Distribute {} => execute_distribute(deps, env),
        ExecuteMsg::Withdraw {} => execute_withdraw(deps, info),
    }
}

pub fn execute_distribute(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    // get contract funds
    let funds = deps
        .querier
        .query_balance(env.contract.address, DENOM.load(deps.storage)?)?;

    let withdrawable_amount = WITHDRAWABLE_AMOUNT.load(deps.storage).unwrap_or_default();

    let distributable_royalties = funds.amount.saturating_sub(withdrawable_amount);
    if distributable_royalties.is_zero() {
        return Err(ContractError::NothingToDistribute {});
    }

    let contributors = CONTRIBUTORS
        .keys(deps.storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<Addr>>>()?;

    let mut distributed_royalties = Uint128::zero();
    for contributor_address in contributors {
        CONTRIBUTORS.update(deps.storage, &contributor_address, |info| {
            // since contributor_address comes from a key of CONTRIBUTORS we should always be able
            // to unwrap().
            let mut info = info.unwrap();

            let contributor_royalties = distributable_royalties * info.percentage_shares;
            // we should rise an error is we have a contributor with 0 royalties to avoid situations
            // where some contibutor receive royalties and other one no.
            if contributor_royalties.is_zero() {
                return Err(ContractError::NotEnoughToDistribute {});
            }

            info.withdrawable_amount = info
                .withdrawable_amount
                .checked_add(contributor_royalties)
                .map_err(ContractError::OverflowErr)?;
            distributed_royalties = distributed_royalties
                .checked_add(contributor_royalties)
                .map_err(ContractError::OverflowErr)?;
            Ok(info)
        })?;
    }

    if distributed_royalties > distributable_royalties {
        return Err(ContractError::NotEnoughToDistribute {});
    }

    WITHDRAWABLE_AMOUNT.update(deps.storage, |amount| -> Result<Uint128, ContractError> {
        amount
            .checked_add(distributed_royalties)
            .map_err(ContractError::OverflowErr)
    })?;

    Ok(Response::new().add_attributes(vec![
        ("action", "execute_distribute"),
        ("amount", &distributed_royalties.to_string()),
    ]))
}

pub fn execute_withdraw(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    // only contributor can withdraw
    let maybe_contributor = CONTRIBUTORS.load(deps.storage, &info.sender);
    if maybe_contributor.is_err() {
        return Err(ContractError::Unauthorized {});
    }

    let mut tokens_to_send: Coin = Coin::new(0u128, DENOM.load(deps.storage)?);
    CONTRIBUTORS.update(deps.storage, &info.sender, |c| {
        let mut contributor = c.unwrap();
        if contributor.withdrawable_amount.is_zero() {
            return Err(ContractError::NothingToWithdraw {});
        }
        tokens_to_send.amount = tokens_to_send
            .amount
            .checked_add(contributor.withdrawable_amount)?;
        // set contributor withdrawable amount to zero since the contract will send their royalties
        contributor.withdrawable_amount = Uint128::zero();
        Ok(contributor)
    })?;

    WITHDRAWABLE_AMOUNT.update(deps.storage, |amount| -> Result<_, ContractError> {
        Ok(amount.checked_sub(tokens_to_send.amount)?)
    })?;

    let msg = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![tokens_to_send.clone()],
    };

    Ok(Response::new()
        .add_attributes(vec![
            ("action", "withdraw"),
            ("contributor", info.sender.as_ref()),
            ("amount", &tokens_to_send.amount.to_string()),
        ])
        .add_message(msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ListContributors { start_after, limit } => {
            to_json_binary(&query_list_contributors(deps, start_after, limit)?)
        }
        QueryMsg::WithdrawableAmount {} => to_json_binary(&query_withdrawable_amount(deps)),
        QueryMsg::DistributableAmount {} => to_json_binary(&query_distributable_amount(deps, env)?),
    }
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn query_list_contributors(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<ContributorListResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let addr = maybe_addr(deps.api, start_after)?;
    let start = addr.as_ref().map(Bound::exclusive);

    let items = CONTRIBUTORS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            item.map(|(addr, data)| ContributorResponse {
                address: addr.into(),
                role: data.role,
                initial_shares: data.initial_shares,
                percentage_shares: data.percentage_shares,
                withdrawable_royalties: data.withdrawable_amount,
            })
        })
        .collect::<StdResult<_>>()?;

    Ok(ContributorListResponse {
        contributors: items,
    })
}

/// Returns the difference between contract balance and the amount of tokens that can be withdrawn as
/// royalties.
pub fn query_distributable_amount(deps: Deps, env: Env) -> StdResult<Uint128> {
    // get contract funds
    let funds = deps
        .querier
        .query_balance(env.contract.address, DENOM.load(deps.storage)?)?;

    let withdrawable_amount = WITHDRAWABLE_AMOUNT.load(deps.storage).unwrap_or_default();

    Ok(funds.amount.saturating_sub(withdrawable_amount))
}

/// Returns the withdrawable amount.
pub fn query_withdrawable_amount(deps: Deps) -> Uint128 {
    WITHDRAWABLE_AMOUNT.load(deps.storage).unwrap_or_default()
}

// -------------------------------------------------------------------------------------------------
// Unit tests
// -------------------------------------------------------------------------------------------------
#[cfg(test)]
mod test {
    use cosmwasm_std::testing::mock_dependencies;

    use super::*;

    #[test]
    fn compute_percentage_shares_works() {
        let mut deps = mock_dependencies();

        let contributor_addr = Addr::unchecked("contributor");
        let role = String::from("dj");

        {
            let total_shares = Uint128::from(100u128);
            let initial_shares = 1u32;

            let percentage = compute_shares_and_store(
                deps.as_mut().storage,
                contributor_addr.clone(),
                initial_shares,
                role.clone(),
                total_shares,
            )
            .unwrap();

            assert_eq!(Decimal::percent(1), percentage)
        }

        {
            let total_shares = Uint128::from(1_000u128);
            let initial_shares = 1u32;

            let percentage = compute_shares_and_store(
                deps.as_mut().storage,
                contributor_addr,
                initial_shares,
                role,
                total_shares,
            )
            .unwrap();

            assert_eq!(Decimal::permille(1), percentage)
        }
    }
}
