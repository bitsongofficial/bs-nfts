use cosmwasm_schema::write_api;

use bs721_curve::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

pub fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ExecuteMsg,
        query: QueryMsg,
    }
}
