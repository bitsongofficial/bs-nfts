use bs721_royalties::msg::ContributorMsg;
use cosmwasm_std::{coin, Addr, Uint128};

use crate::msg::ConfigResponse;

use super::suite::TestSuiteBuilder;

/// Helper function to create a contributor message.
fn mock_contributor_msg(
    role: impl ToString,
    shares: u32,
    address: impl ToString,
) -> ContributorMsg {
    ContributorMsg {
        role: role.to_string(),
        shares,
        address: address.to_string(),
    }
}

#[test]
fn instantiate() {
    let suite = TestSuiteBuilder::new()
        .with_contributors(vec![mock_contributor_msg(
            "creator",
            100,
            "contributor0000",
        )])
        .build();

    let resp = suite.query_config();

    // ensure created contract addresses are correctly saved in the state
    assert_eq!(
        resp.bs721_base,
        Some(Addr::unchecked("contract1")),
        "expected bs721 base as second contract stored and saved in the state"
    );

    assert_eq!(
        resp.bs721_royalties,
        Some(Addr::unchecked("contract2")),
        "expected bs721 royalties as third contract stored and saved in the state"
    )
}

#[test]
fn mint_single_no_referral() {
    let mut suite = TestSuiteBuilder::new()
        .with_default_contributors(vec![1])
        .with_funds("address1", &[coin(1_000, "ubtsg")])
        .with_price(coin(1, "ubtsg"))
        .build();

    suite
        .mint("address1", None, 1, Some(coin(1, "ubtsg")))
        .unwrap();

    // retrieve royalties contract to query it
    let royalties_address = suite.query_config().bs721_royalties.unwrap();

    assert_eq!(
        suite.query_address_balance(royalties_address, "ubtsg").amount,
        Uint128::one(),
        "expected to have the royalties contract balance equal to the price of a single NFT"
    );

    suite
        .mint("address1", None, 1, Some(coin(1, "ubtsg")))
        .unwrap_err();
}

#[test]
fn mint_single_with_referral() {
    let mut suite = TestSuiteBuilder::new()
        .with_default_contributors(vec![1])
        .with_funds("address1", &[coin(1_000, "ubtsg")])
        .with_referral_fee_bps(1_000)
        .with_price(coin(10, "ubtsg"))
        .build();

    let referral = Some("referral".to_string());
    suite
        .mint("address1", referral.clone(), 1, Some(coin(10, "ubtsg")))
        .unwrap();

    // retrieve royalties contract to query it
    let royalties_address = suite.query_config().bs721_royalties.unwrap();

    assert_eq!(
        suite.query_address_balance(referral.unwrap(), "ubtsg").amount,
        Uint128::one(),
        "expected to have the referral address balance equal to the 10% of the NFT price"
    );

    assert_eq!(
        suite.query_address_balance(royalties_address, "ubtsg").amount,
        Uint128::new(9),
        "expected to have the royalties contract balance equal to the 90% of the NFT price"
    );
}

#[test]
fn mint_multiple() {
    let mut suite = TestSuiteBuilder::new()
        .with_default_contributors(vec![1])
        .with_funds("address1", &[coin(1_000, "ubtsg")])
        .with_price(coin(1, "ubtsg"))
        .with_party_type(crate::msg::PartyType::MaxEdition(10))
        .build();

    suite
        .mint("address1", None, 3, Some(coin(3, "ubtsg")))
        .unwrap();

    // retrieve royalties contract to query it
    let config = suite.query_config();
    let royalties_address = config.bs721_royalties.unwrap();

    assert_eq!(
        suite.query_address_balance(royalties_address, "ubtsg").amount,
        Uint128::new(3),
        "expected to have the royalties contract balance equal to the price of 3 NFT"
    );

    assert_eq!(
        vec!["1", "2", "3"],
        suite.query_nft_token(config.bs721_base.unwrap(), "address1"),
        "expected 3 nft with sequential ids starting from 1"
    );
}
