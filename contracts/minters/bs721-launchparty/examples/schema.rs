use cosmwasm_schema::write_api;

use bs721_launchparty::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

pub fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ExecuteMsg,
        query: QueryMsg,
    }
}
