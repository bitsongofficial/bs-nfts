use abstract_interface::{Abstract, AccountDetails};
use clap::Parser;
use cw_orch::{daemon::DaemonBuilder, prelude::*};
use reqwest::Url;
use tokio::{net::TcpStream, runtime::Runtime};

pub const MNEMONIC: &str = "";

#[derive(Parser, Default, Debug)]
#[command(author, version, about, long_about = None)]
struct Arguments {
    /// Network Id to deploy on
    #[arg(short, long)]
    network: String,
    #[arg(short, long)]
    nft_addr: String,
    #[arg(short, long)]
    id: String,
}

pub fn main() -> cw_orch::anyhow::Result<()> {
    dotenv::dotenv()?;
    env_logger::init();
    let args = Arguments::parse();
    let rt = Runtime::new()?;

    let bitsong_chain = match args.network.as_str() {
        "main" => scripts::framework::networks::BITSONG_MAINNET.to_owned(),
        "testnet" => scripts::framework::networks::BITSONG_TESTNET.to_owned(),
        "local" => scripts::framework::networks::LOCAL_NETWORK1.to_owned(),
        _ => panic!("Invalid network"),
    };
    let urls = bitsong_chain.grpc_urls.to_vec();
    for url in urls {
        rt.block_on(ping_grpc(&url))?;
    }

    let chain = DaemonBuilder::new(bitsong_chain.clone())
        .handle(rt.handle())
        .mnemonic(MNEMONIC)
        .build()?;

    let deployment = match Abstract::load_from(chain) {
        Ok(deployment) => {
            // write_deployment(&deployment_status)?;
            deployment
        }
        Err(e) => {
            // write_deployment(&deployment_status)?;
            return Err(e.into());
        }
    };

    // creates an nft-owned account
    deployment.account_factory.create_new_account(
        AccountDetails {
            name: "NFT Owned Account".into(),
            description: Some("account owned via nft token".into()),
            link: None,
            namespace: None,
            base_asset: None,
            install_modules: vec![],
            account_id: None,
        },
        abstract_client::GovernanceDetails::NFT {
            collection_addr: args.nft_addr,
            token_id: args.id,
        },
        None,
    )?;

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
