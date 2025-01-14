#![allow(unused_imports)]
use std::iter;

use abstract_interface::{Abstract, AbstractIbc};
use cw_orch::{
    daemon::{
        networks::{neutron::NEUTRON_NETWORK, ARCHWAY_1, JUNO_1, OSMOSIS_1, PHOENIX_1},
        DaemonState,
    },
    environment::ChainKind,
    prelude::*,
    tokio::runtime::{Handle, Runtime},
};
use scripts::framework::networks::BITSONG_MAINNET;
pub const NETWORK: ChainInfo = BITSONG_MAINNET;

pub const MNEMONIC: &str = "";

/// Script to deploy the IBC modules on a chain.
/// Currently deployed by abstract, so only used on chains where IBC is not present.
pub fn main() -> cw_orch::anyhow::Result<()> {
    dotenv::dotenv()?;
    env_logger::init();

    let runtime = Runtime::new()?;
    let first_daemon = get_daemon(NETWORK, runtime.handle(), Some(MNEMONIC.to_string()), None)?;
    let daemons = vec![
        get_daemon(JUNO_1, runtime.handle(), Some(MNEMONIC.to_string()), Some(first_daemon.state()))?,
        get_daemon(
            BITSONG_MAINNET,
            runtime.handle(),
            Some(MNEMONIC.to_string()),
            Some(first_daemon.state()),
        )?,
        // get_daemon(
        //     OSMOSIS_1,
        //     runtime.handle(),
        //     Some(std::env::var("OSMOSIS_MNEMONIC")?),
        //     Some(first_daemon.state()),
        // )?,
    ];

    for daemon in daemons.into_iter().chain(iter::once(first_daemon)) {
        deploy_host_and_client(daemon)?;
    }

    Ok(())
}

fn get_daemon(
    chain: ChainInfo,
    handle: &Handle,
    mnemonic: Option<String>,
    state: Option<DaemonState>,
) -> cw_orch::anyhow::Result<Daemon> {
    let mut builder = DaemonBuilder::new(chain);
    builder.handle(handle);
    if let Some(state) = state {
        builder.state(state);
    }
    if let Some(mnemonic) = mnemonic {
        builder.mnemonic(mnemonic);
    }
    Ok(builder.build()?)
}

pub fn deploy_host_and_client<Chain: CwEnv>(chain: Chain) -> cw_orch::anyhow::Result<()> {
    let abs = Abstract::load_from(chain.clone())?;
    let ibc_infra = AbstractIbc::new(&chain);
    ibc_infra.upload()?;
    ibc_infra.instantiate(&abs, &chain.sender_addr())?;
    ibc_infra.register(&abs.version_control)?;

    abs.version_control.approve_any_abstract_modules()?;

    Ok(())
}
