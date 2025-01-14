/// SUPPORTED NETWORK CONFIGS
///  
/// 
use cw_orch::environment::{ChainKind, NetworkInfo};
/// Add more chains in SUPPORTED_CHAINS to include in account framework instance.
use cw_orch::prelude::{networks::UNI_6, *};

pub const SUPPORTED_CHAINS: &[ChainInfo] = &[UNI_6, BITSONG_TESTNET];
pub const BITSONG_SUPPORTED_NETWORKS: &[ChainInfo] = &SUPPORTED_CHAINS;

pub const GAS_TO_DEPLOY: u64 = 60_000_000;

/// A helper function to retrieve a [`ChainInfo`] struct for a given chain-id.
/// supported chains are defined by the `SUPPORTED_CHAINS` variable
pub fn bitsong_parse_networks(net_id: &str) -> Result<ChainInfo, String> {
    BITSONG_SUPPORTED_NETWORKS
        .iter()
        .find(|net| net.chain_id == net_id)
        .cloned()
        .ok_or(format!("Network not found: {}", net_id))
}

/// Bitsong: <https://github.com/cosmos/chain-registry/blob/master/bitsong/chain.json>
pub const BITSONG_NETWORK: NetworkInfo = NetworkInfo {
    chain_name: "Bitsong",
    pub_address_prefix: "bitsong",
    coin_type: 639u32,
};

pub const BITSONG_MAINNET: ChainInfo = ChainInfo {
    kind: ChainKind::Mainnet,
    chain_id: "bitsong-2b",
    gas_denom: "ubtsg",
    gas_price: 0.025,
    grpc_urls: &["http://grpc"],
    network_info: BITSONG_NETWORK,
    lcd_url: None,
    fcd_url: None,
};

pub const BITSONG_TESTNET: ChainInfo = ChainInfo {
    kind: ChainKind::Testnet,
    chain_id: "bobnet",
    gas_denom: "ubtsg",
    gas_price: 0.025,
    grpc_urls: &["http://grpc"],
    network_info: BITSONG_NETWORK,
    lcd_url: None,
    fcd_url: None,
};

// Localnet: <https://github.com/cosmos/chain-registry/blob/master/bitsong/chain.json>
const LOCAL_NET: NetworkInfo = NetworkInfo {
    chain_name: "Local Network",
    pub_address_prefix: "mock",
    coin_type: 114u32,
};
pub const LOCAL_NETWORK1: ChainInfo = ChainInfo {
    kind: ChainKind::Testnet,
    chain_id: "local-1",
    gas_denom: "ueret",
    gas_price: 0.025,
    grpc_urls: &["http://grpc"],
    network_info: BITSONG_NETWORK,
    lcd_url: None,
    fcd_url: None,
};
pub const LOCAL_NETWORK2: ChainInfo = ChainInfo {
    kind: ChainKind::Testnet,
    chain_id: "local-2",
    gas_denom: "uskeret",
    gas_price: 0.025,
    grpc_urls: &["http://grpc"],
    network_info: LOCAL_NET,
    lcd_url: None,
    fcd_url: None,
};
