use abstract_interface::Abstract;
use cw_orch::daemon::DaemonState;
use cw_orch::prelude::*;
use cw_orch::tokio::runtime::Handle;

use cw_orch_interchain::prelude::*;
use networks::JUNO_1;
use scripts::framework::networks::BITSONG_MAINNET;
use tokio::runtime::Runtime;

/// Connect IBC between two chains.
pub fn main() -> cw_orch::anyhow::Result<()> {
    dotenv::dotenv()?;
    env_logger::init();

    let chains = vec![
        (JUNO_1, None),
        (BITSONG_MAINNET, None),
        // (OSMOSIS_1, Some(std::env::var("OSMOSIS_MNEMONIC")?)),
    ];
    let runtime = Runtime::new()?;

    let src_chain = &chains[1];
    let dst_chain = &chains[0];

    connect(src_chain.clone(), dst_chain.clone(), runtime.handle())?;

    Ok(())
}

pub fn get_daemon(
    chain: ChainInfo,
    handle: &Handle,
    mnemonic: Option<String>,
    deployment_id: Option<String>,
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
    if let Some(deployment_id) = deployment_id {
        builder.deployment_id(deployment_id);
    }
    Ok(builder.build()?)
}

pub fn get_deployment_id(src_chain: &ChainInfo, dst_chain: &ChainInfo) -> String {
    format!("{}-->{}", src_chain.chain_id, dst_chain.chain_id)
}

pub fn connect(
    (src_chain, src_mnemonic): (ChainInfo, Option<String>),
    (dst_chain, dst_mnemonic): (ChainInfo, Option<String>),
    handle: &Handle,
) -> cw_orch::anyhow::Result<()> {
    let src_daemon = get_daemon(src_chain.clone(), handle, src_mnemonic.clone(), None, None)?;
    let dst_daemon = get_daemon(
        dst_chain.clone(),
        handle,
        dst_mnemonic,
        None,
        Some(src_daemon.state()),
    )?;

    let src_abstract = Abstract::load_from(src_daemon.clone())?;
    let dst_abstract = Abstract::load_from(dst_daemon.clone())?;

    let _src_polytone_daemon = get_daemon(
        src_chain.clone(),
        handle,
        src_mnemonic,
        Some(get_deployment_id(&src_chain, &dst_chain)),
        Some(src_daemon.state()),
    )?;

    let interchain =
        DaemonInterchainEnv::from_daemons(vec![src_daemon, dst_daemon], &ChannelCreationValidator);
    src_abstract.connect_to(&dst_abstract, &interchain)?;

    Ok(())
}
