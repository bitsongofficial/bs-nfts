use cw_orch::prelude::*;
use scripts::framework::networks::{GAS_TO_DEPLOY, SUPPORTED_CHAINS};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeploymentStatus {
    pub chain_ids: Vec<String>,
    pub success: bool,
}

pub async fn assert_wallet_balance(mut chains: Vec<ChainInfoOwned>) -> Vec<ChainInfoOwned> {
    if chains.is_empty() {
        chains = SUPPORTED_CHAINS.iter().cloned().map(Into::into).collect();
    }
    // check that the wallet has enough gas on all the chains we want to support
    for chain_info in &chains {
        let chain = DaemonAsyncBuilder::new(chain_info.clone())
            .build()
            .await
            .unwrap();

        let gas_denom = chain.state().chain_data.gas_denom.clone();
        let gas_price = chain.state().chain_data.gas_price;
        let fee = (GAS_TO_DEPLOY as f64 * gas_price) as u128;
        let bank = queriers::Bank::new_async(chain.channel());
        let balance = bank
            ._balance(chain.sender_addr(), Some(gas_denom.clone()))
            .await
            .unwrap()
            .clone()[0]
            .clone();

        log::debug!(
            "Checking balance {} on chain {}, address {}. Expecting {}{}",
            balance.amount,
            chain_info.chain_id,
            chain.sender_addr(),
            fee,
            gas_denom
        );
        if fee > balance.amount.u128() {
            panic!("Not enough funds on chain {} to deploy the contract. Needed: {}{} but only have: {}{}", chain_info.chain_id, fee, gas_denom, balance.amount, gas_denom);
        }
        // check if we have enough funds
    }

    chains
}


pub fn main() {}