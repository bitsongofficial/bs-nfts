use std::net::TcpStream;

use abstract_interface::Abstract;

use abstract_std::objects::gov_type::GovernanceDetails;
use clap::Parser;
use cw_orch::prelude::*;
use reqwest::Url;
use scripts::framework::assert_wallet_balance;
use tokio::runtime::Runtime;

use cw_orch_polytone::Polytone;

pub const ABSTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Script to deploy Abstract & polytone to a new network provided by commmand line arguments
pub fn manual_deploy(network: ChainInfoOwned) -> anyhow::Result<()> {
    let rt = Runtime::new()?;

    rt.block_on(assert_wallet_balance(vec![network.clone()]));

    let urls = network.grpc_urls.to_vec();
    for url in urls {
        rt.block_on(ping_grpc(&url))?;
    }

    let chain = DaemonBuilder::new(network.clone())
        .handle(rt.handle())
        .build()?;

    let sender = chain.sender_addr();

    // Abstract
    let _abstr = match Abstract::load_from(chain.clone()) {
        Ok(deployed) => deployed,
        Err(_) => {
            let abs = Abstract::deploy_on(chain.clone(), sender.to_string())?;
            // Create the Abstract Account because it's needed for the fees for the dex module
            abs.account_factory
                .create_default_account(GovernanceDetails::Monarchy {
                    monarch: sender.to_string(),
                })?;

            abs
        }
    };

    // Attempt to load or deploy Polytone based on condition check
    let _polytone = match Polytone::load_from(chain.clone()) {
        Ok(deployed) => {
            // Check if the address property of deployed Polytone indicates it's properly deployed
            match deployed.note.address() {
                Ok(_) => deployed, // Use the deployed instance if check is successful
                Err(CwOrchError::AddrNotInStore(_)) => {
                    // If the check fails, deploy a new instance instead of returning an error
                    Polytone::deploy_on(chain.clone(), Empty {})?
                }
                Err(e) => return Err(e.into()), // Return any other error
            }
        }
        // If Polytone is not loaded, deploy a new one
        Err(_) => Polytone::deploy_on(chain.clone(), Empty {})?,
    };

    Ok(())
}

async fn ping_grpc(url_str: &str) -> anyhow::Result<()> {
    let parsed_url = Url::parse(url_str)?;

    let host = parsed_url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("No host in url"))?;

    let port = parsed_url.port_or_known_default().ok_or_else(|| {
        anyhow::anyhow!(
            "No port in url, and no default for scheme {:?}",
            parsed_url.scheme()
        )
    })?;
    let socket_addr = format!("{}:{}", host, port);

    let _ = TcpStream::connect(socket_addr);
    Ok(())
}

#[derive(Parser, Default, Debug)]
#[command(author, version, about, long_about = None)]
struct Arguments {
    /// network to deploy on: main, testnet, local
    #[arg(long)]
    network: String,
}

pub fn main() {
    dotenv().ok();
    env_logger::init();

    use dotenv::dotenv;

    let args = Arguments::parse();

    let bitsong_chain = match args.network.as_str() {
        "main" => scripts::framework::networks::BITSONG_MAINNET.to_owned(),
        "testnet" => scripts::framework::networks::BITSONG_TESTNET.to_owned(),
        "local" => scripts::framework::networks::LOCAL_NETWORK1.to_owned(),
        _ => panic!("Invalid network"),
    };

    if let Err(ref err) = manual_deploy(bitsong_chain.into()) {
        log::error!("{}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| log::error!("because: {}", cause));

        // The backtrace is not always generated. Try to run this example
        // with `$env:RUST_BACKTRACE=1`.
        //    if let Some(backtrace) = e.backtrace() {
        //        log::debug!("backtrace: {:?}", backtrace);
        //    }

        ::std::process::exit(1);
    }
}
