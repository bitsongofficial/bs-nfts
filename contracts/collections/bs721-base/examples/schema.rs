use cosmwasm_schema::write_api;
use cosmwasm_std::Empty;

use bs721_base::{ExecuteMsg, InstantiateMsg, QueryMsg};

pub fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ExecuteMsg<Empty>,
        query: QueryMsg<Empty>,
    }
}
