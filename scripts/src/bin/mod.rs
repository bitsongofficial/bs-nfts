pub mod clone_mock_test;
pub mod connect_ibc;
pub mod create_remote_account;
pub mod deploy_ibc;
pub mod deploy_modules;
pub mod full_deploy;
pub mod ibc_test;
pub mod create_nft_account;
pub mod manual_deploy;
pub mod helper;
// pub mod migrate;

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    name: String,
}

fn main() {
    let args = Args::parse();
    println!("Hello, {}!", args.name);
}
