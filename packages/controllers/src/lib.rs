mod hooks;
mod init;

/// Hook library for having contract react upon another contract reacting.
pub use hooks::{HookError, Hooks, HooksResponse};
/// Helpers to define contract admin & creator.
pub use init::{Admin, ContractInstantiateMsg};
