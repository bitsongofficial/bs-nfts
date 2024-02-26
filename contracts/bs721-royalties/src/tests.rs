use cosmwasm_std::{
    coin, coins,
    testing::{mock_dependencies, mock_env, mock_info},
    Attribute, BankMsg, CosmosMsg, Decimal, DepsMut, Uint128,
};

use crate::{
    contract::{execute, query_distributable_amount, query_withdrawable_amount},
    state::DENOM as STATE_DENOM,
};
use crate::{
    contract::{instantiate, query_list_contributors},
    msg::{ContributorListResponse, ContributorMsg, ContributorResponse, InstantiateMsg},
    ContractError,
};
use crate::{msg::ExecuteMsg, state::WITHDRAWABLE_AMOUNT};

const DENOM: &str = "ubtsg";
const CONTRIBUTOR1: &str = "contributor1";
const CONTRIBUTOR2: &str = "contributor2";
const CONTRIBUTOR3: &str = "contributor3";

fn init(deps: DepsMut) {
    let msg = InstantiateMsg {
        denom: DENOM.into(),
        contributors: vec![
            ContributorMsg {
                address: CONTRIBUTOR1.into(),
                role: "role".into(),
                shares: 10,
            },
            ContributorMsg {
                address: CONTRIBUTOR2.into(),
                role: "role".into(),
                shares: 20,
            },
            ContributorMsg {
                address: CONTRIBUTOR3.into(),
                role: "role".into(),
                shares: 30,
            },
        ],
    };

    let info = mock_info("creator", &[]);
    instantiate(deps, mock_env(), info, msg).unwrap();
}

/// Helper function to initialize the contract with a number of contributors equal to the number of shares
/// passed as input.
fn init_with_shares(deps: DepsMut, shares: Vec<u32>) {
    let mut contributors: Vec<ContributorMsg> = Vec::with_capacity(shares.len());
    for (idx, curr_shares) in shares.into_iter().enumerate() {
        contributors.push(ContributorMsg {
            role: String::from(""),
            shares: curr_shares,
            address: format!("address{}", idx),
        })
    }
    let msg = InstantiateMsg {
        denom: DENOM.into(),
        contributors,
    };

    let info = mock_info("creator", &[]);
    instantiate(deps, mock_env(), info, msg).unwrap();
}

// -------------------------------------------------------------------------------------------------
// Instantiate
// -------------------------------------------------------------------------------------------------
#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies();
    init(deps.as_mut());
}

// -------------------------------------------------------------------------------------------------
// Distribute shares
// -------------------------------------------------------------------------------------------------
#[test]
fn distribute_shares_fails() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    init_with_shares(deps.as_mut(), vec![10]);

    let info = mock_info("random_user", &[]);
    let msg = ExecuteMsg::Distribute {};

    {
        let err = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap_err();
        assert_eq!(
            ContractError::NothingToDistribute {},
            err,
            "expected error since contract has not funds to distribute"
        );
    }

    {
        deps.querier
            .update_balance(env.contract.address.clone(), coins(1_000, "NOT_DENOM"));
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(
            ContractError::NothingToDistribute {},
            err,
            "expected error since contract has not funds of correct denom to distribute"
        );
    }
}

#[test]
fn not_nough_to_distribute() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    init_with_shares(deps.as_mut(), vec![99, 1]);

    let info = mock_info("random_user", &[]);
    let msg = ExecuteMsg::Distribute {};

    {
        deps.querier
            .update_balance(env.contract.address.clone(), coins(99, DENOM));
        let resp = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(
            resp,
            ContractError::NotEnoughToDistribute {},
            "expected to fail since 1% of 99 is approximated to zero"
        );

        let query_resp = query_withdrawable_amount(deps.as_ref());
        assert_eq!(query_resp, Uint128::new(0), "expected nothing to withdraw");
    }
}

#[test]
fn distribute_shares_single() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    init_with_shares(deps.as_mut(), vec![10]);

    let info = mock_info("random_user", &[]);
    let msg = ExecuteMsg::Distribute {};

    {
        deps.querier
            .update_balance(env.contract.address.clone(), coins(1_000, DENOM));
        let resp = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        assert_eq!(
            resp.attributes[1],
            Attribute {
                key: "amount".to_owned(),
                value: "1000".to_owned()
            },
            "expected to succed and return an amount equal to contract balance"
        );

        let query_resp = query_withdrawable_amount(deps.as_ref());
        assert_eq!(
            query_resp,
            Uint128::new(1000),
            "expected all initial balance as withdrawable"
        )
    }

    {
        deps.querier
            .update_balance(env.contract.address.clone(), coins(2_000, DENOM));
        execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::NothingToDistribute {},
            "expected to fail since after first distribution the contract has no more funds"
        );

        let query_resp = query_withdrawable_amount(deps.as_ref());
        assert_eq!(
            query_resp,
            Uint128::new(2000),
            "expected 1_000 + 1_000 as withdrawable"
        )
    }
}

#[test]
fn distribute_two_contributor() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    init_with_shares(deps.as_mut(), vec![99, 1]);

    let info = mock_info("random_user", &[]);
    let msg = ExecuteMsg::Distribute {};

    {
        deps.querier
            .update_balance(env.contract.address.clone(), coins(101, DENOM));
        let resp = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(
            resp.attributes[1],
            Attribute {
                key: "amount".to_owned(),
                value: "100".to_owned()
            },
            "expected to succed and return an amount equal to contract balance"
        );

        let query_resp = query_withdrawable_amount(deps.as_ref());
        assert_eq!(
            query_resp,
            Uint128::new(100),
            "expected distributed token to withdraw"
        );

        let query_resp = query_distributable_amount(deps.as_ref(), env).unwrap();
        assert_eq!(
            query_resp,
            Uint128::new(1),
            "expected only one token to be distributed"
        );
    }
}

#[test]
fn distribute_shares_multiple() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    init_with_shares(deps.as_mut(), vec![10, 20, 30, 40]);

    let info = mock_info("random_user", &[]);
    let msg = ExecuteMsg::Distribute {};

    {
        deps.querier
            .update_balance(env.contract.address.clone(), coins(1_000, DENOM));
        let resp = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        assert_eq!(
            resp.attributes[1],
            Attribute {
                key: "amount".to_owned(),
                value: "1000".to_owned()
            },
            "expected to succed and return an amount equal to contract balance"
        );

        let query_resp = query_withdrawable_amount(deps.as_ref());
        assert_eq!(
            query_resp,
            Uint128::new(1000),
            "expected all initial balance as withdrawable"
        );

        let query_resp = query_list_contributors(deps.as_ref(), None, None).unwrap();
        assert_eq!(
            query_resp.contributors[0].withdrawable_royalties,
            Uint128::new(100)
        );
        let query_resp = query_list_contributors(deps.as_ref(), None, None).unwrap();
        assert_eq!(
            query_resp.contributors[1].withdrawable_royalties,
            Uint128::new(200)
        );
        let query_resp = query_list_contributors(deps.as_ref(), None, None).unwrap();
        assert_eq!(
            query_resp.contributors[2].withdrawable_royalties,
            Uint128::new(300)
        );
        let query_resp = query_list_contributors(deps.as_ref(), None, None).unwrap();
        assert_eq!(
            query_resp.contributors[3].withdrawable_royalties,
            Uint128::new(400)
        );
    }

    {
        deps.querier
            .update_balance(env.contract.address.clone(), coins(2_000, DENOM));
        execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        let err = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::NothingToDistribute {},
            "expected to fail since after first distribution the contract has no more funds"
        );

        let query_resp = query_distributable_amount(deps.as_ref(), env).unwrap();
        assert_eq!(
            query_resp,
            Uint128::zero(),
            "expected nothing to distribute"
        );
    }
}

// -------------------------------------------------------------------------------------------------
// Withdraw royalties
// -------------------------------------------------------------------------------------------------
#[test]
fn withdraw_royalties_fails() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    init(deps.as_mut());

    let withdraw_msg = ExecuteMsg::Withdraw {};

    {
        let info = mock_info("random_user", &[]);
        let err = execute(deps.as_mut(), env.clone(), info, withdraw_msg.clone()).unwrap_err();
        assert_eq!(
            err,
            ContractError::Unauthorized {},
            "expected to fail since only contributors can withdraw"
        )
    }

    {
        let info = mock_info(CONTRIBUTOR1, &[]);
        let err = execute(deps.as_mut(), env, info, withdraw_msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::NothingToWithdraw {},
            "expected to fail since contributors has nothing to withdraw"
        )
    }
}

#[test]
fn withdraw_royalties_single() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    init_with_shares(deps.as_mut(), vec![1]);

    let info = mock_info("address0", &[]);
    let distribute_msg = ExecuteMsg::Distribute {};
    let withdraw_msg = ExecuteMsg::Withdraw {};

    {
        // update contract balance a contributor can withdraw
        deps.querier
            .update_balance(env.contract.address.clone(), coins(1_000, DENOM));
        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            distribute_msg,
        )
        .unwrap();

        let resp = execute(deps.as_mut(), env, info, withdraw_msg).unwrap();
        assert_eq!(
            resp.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "address0".into(),
                amount: vec![coin(1_000, DENOM.to_string())],
            })
        );

        let query_resp = query_withdrawable_amount(deps.as_ref());
        assert_eq!(
            query_resp,
            Uint128::zero(),
            "expected withdrawable amount zero after withdraw"
        );

        let query_resp = query_list_contributors(deps.as_ref(), None, None).unwrap();
        assert_eq!(
            query_resp.contributors[0].withdrawable_royalties,
            Uint128::zero(),
            "expected withdrawable royalties zero after withdraw"
        )
    }
}

#[test]
fn withdraw_royalties_multiple() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    init_with_shares(deps.as_mut(), vec![10, 10]);

    let distribute_msg = ExecuteMsg::Distribute {};
    let withdraw_msg = ExecuteMsg::Withdraw {};

    // update contract balance a contributor can withdraw
    deps.querier
        .update_balance(env.contract.address.clone(), coins(1_000, DENOM));

    let info = mock_info("address0", &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        distribute_msg,
    )
    .unwrap();

    let resp = execute(
        deps.as_mut(),
        env.clone(),
        info,
        withdraw_msg.clone(),
    )
    .unwrap();
    assert_eq!(
        resp.messages[0].msg,
        CosmosMsg::Bank(BankMsg::Send {
            to_address: "address0".into(),
            amount: vec![coin(500, DENOM.to_string())],
        })
    );

    let query_resp = query_withdrawable_amount(deps.as_ref());
    assert_eq!(
        query_resp,
        Uint128::new(500),
        "expected withdrawable amount half initial amount"
    );

    let query_resp = query_list_contributors(deps.as_ref(), None, None).unwrap();
    assert_eq!(
        query_resp.contributors[0].withdrawable_royalties,
        Uint128::zero(),
        "expected withdrawable royalties of address0 zero after withdraw"
    );

    // we can still withdraw from second contributor
    let info = mock_info("address1", &[]);

    let resp = execute(deps.as_mut(), env, info, withdraw_msg).unwrap();
    assert_eq!(
        resp.messages[0].msg,
        CosmosMsg::Bank(BankMsg::Send {
            to_address: "address1".into(),
            amount: vec![coin(500, DENOM.to_string())],
        })
    );

    let query_resp = query_withdrawable_amount(deps.as_ref());
    assert_eq!(
        query_resp,
        Uint128::zero(),
        "expected withdrawable amount zero after all contributors withdraw"
    );

    let query_resp = query_list_contributors(deps.as_ref(), None, None).unwrap();
    assert_eq!(
        query_resp.contributors[1].withdrawable_royalties,
        Uint128::zero(),
        "expected withdrawable royalties of address1 zero after withdraw"
    );
}

#[test]
fn mixed_distribute_and_withdraw() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    init_with_shares(deps.as_mut(), vec![10, 10]);

    let distribute_msg = ExecuteMsg::Distribute {};
    let withdraw_msg = ExecuteMsg::Withdraw {};

    // update contract balance a contributor can withdraw
    deps.querier
        .update_balance(env.contract.address.clone(), coins(1_000, DENOM));

    // first distribution
    let info = mock_info("address0", &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        distribute_msg.clone(),
    )
    .unwrap();

    // first withdraw from contributor0
    execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        withdraw_msg.clone(),
    )
    .unwrap();

    // we have to update contract balance since bank messages are not executed. We will have:
    // 1_000 (initial) - 500 (address0 withdraw) + 1_000 (new royalties to distribtue)
    deps.querier
        .update_balance(env.contract.address.clone(), coins(1_500, DENOM));

    // second distribution
    execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        distribute_msg,
    )
    .unwrap();

    // second withdraw from contributor0
    let resp = execute(
        deps.as_mut(),
        env.clone(),
        info,
        withdraw_msg.clone(),
    )
    .unwrap();
    assert_eq!(
        resp.messages[0].msg,
        CosmosMsg::Bank(BankMsg::Send {
            to_address: "address0".into(),
            amount: vec![coin(500, DENOM.to_string())],
        })
    );

    let info = mock_info("address1", &[]);

    // first withdraw from contributor1
    let resp = execute(deps.as_mut(), env, info, withdraw_msg).unwrap();
    assert_eq!(
        resp.messages[0].msg,
        CosmosMsg::Bank(BankMsg::Send {
            to_address: "address1".into(),
            amount: vec![coin(1_000, DENOM.to_string())],
        })
    );

    let query_resp = query_withdrawable_amount(deps.as_ref());
    assert_eq!(
        query_resp,
        Uint128::zero(),
        "expected withdrawable amount zero after all contributors withdraw"
    );
}

// -------------------------------------------------------------------------------------------------
// Queries
// -------------------------------------------------------------------------------------------------
#[test]
fn query_withdrawable_amount_works() {
    let mut deps = mock_dependencies();

    {
        WITHDRAWABLE_AMOUNT
            .save(deps.as_mut().storage, &Uint128::zero())
            .unwrap();

        let query_resp = query_withdrawable_amount(deps.as_ref());
        assert_eq!(
            query_resp,
            Uint128::zero(),
            "expected zero since just initialized"
        );
    }

    {
        WITHDRAWABLE_AMOUNT
            .save(deps.as_mut().storage, &Uint128::new(1_000))
            .unwrap();

        let query_resp = query_withdrawable_amount(deps.as_ref());
        assert_eq!(query_resp, Uint128::new(1_000), "expected 1_000");
    }
}

#[test]
fn query_distributable_amount_works() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    {
        STATE_DENOM
            .save(deps.as_mut().storage, &String::from(DENOM))
            .unwrap();
        WITHDRAWABLE_AMOUNT
            .save(deps.as_mut().storage, &Uint128::zero())
            .unwrap();

        let query_resp = query_distributable_amount(deps.as_ref(), env.clone()).unwrap();
        assert_eq!(
            query_resp,
            Uint128::zero(),
            "expected zero since contract as no funds"
        );
    }

    {
        WITHDRAWABLE_AMOUNT
            .save(deps.as_mut().storage, &Uint128::new(1))
            .unwrap();

        let query_resp = query_distributable_amount(deps.as_ref(), env.clone()).unwrap();
        assert_eq!(
            query_resp,
            Uint128::zero(),
            "expected zero also if contract has less funds than distributable"
        );
    }

    {
        deps.querier
            .update_balance(env.contract.address.clone(), coins(1_000, DENOM));
        WITHDRAWABLE_AMOUNT
            .save(deps.as_mut().storage, &Uint128::zero())
            .unwrap();

        let query_resp = query_distributable_amount(deps.as_ref(), env).unwrap();
        assert_eq!(
            query_resp,
            Uint128::new(1_000),
            "expected the difference between balance and withdrawable amount"
        );
    }
}

// TODO: should we test query without execution
#[test]
fn test_query_list_contributors() {
    let mut deps = mock_dependencies();
    init(deps.as_mut());

    let contributors = query_list_contributors(deps.as_ref(), None, None).unwrap();
    assert_eq!(contributors.contributors.len(), 3);

    assert_eq!(
        contributors,
        ContributorListResponse {
            contributors: vec![
                ContributorResponse {
                    address: CONTRIBUTOR1.into(),
                    role: "role".into(),
                    initial_shares: 10,
                    percentage_shares: Decimal::from_ratio(10u128, 60u128),
                    withdrawable_royalties: Uint128::zero(),
                },
                ContributorResponse {
                    address: CONTRIBUTOR2.into(),
                    role: "role".into(),
                    initial_shares: 20,
                    percentage_shares: Decimal::from_ratio(20u128, 60u128),
                    withdrawable_royalties: Uint128::zero(),
                },
                ContributorResponse {
                    address: CONTRIBUTOR3.into(),
                    role: "role".into(),
                    initial_shares: 30,
                    percentage_shares: Decimal::from_ratio(30u128, 60u128),
                    withdrawable_royalties: Uint128::zero(),
                },
            ]
        }
    );

    let contributors =
        query_list_contributors(deps.as_ref(), Some(CONTRIBUTOR1.into()), None).unwrap();
    assert_eq!(contributors.contributors.len(), 2);

    let contributors =
        query_list_contributors(deps.as_ref(), Some(CONTRIBUTOR2.into()), None).unwrap();
    assert_eq!(contributors.contributors.len(), 1);

    let contributors =
        query_list_contributors(deps.as_ref(), Some(CONTRIBUTOR3.into()), None).unwrap();
    assert_eq!(contributors.contributors.len(), 0);

    let contributors =
        query_list_contributors(deps.as_ref(), Some(CONTRIBUTOR1.into()), Some(1)).unwrap();
    assert_eq!(contributors.contributors.len(), 1);

    assert_eq!(
        contributors,
        ContributorListResponse {
            contributors: vec![ContributorResponse {
                address: CONTRIBUTOR2.into(),
                role: "role".into(),
                initial_shares: 20,
                percentage_shares: Decimal::from_ratio(20u128, 60u128),
                withdrawable_royalties: Uint128::zero(),
            }]
        }
    )
}
