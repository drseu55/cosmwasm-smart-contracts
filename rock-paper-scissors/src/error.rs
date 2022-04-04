use cosmwasm_std::StdError;
use cw_controllers::{AdminError, HookError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Custom Error val: {val:?}")]
    CustomError { val: String },

    #[error("Address is not valid")]
    NotValidAddres {},

    #[error("No admin found")]
    Admin(#[from] AdminError),

    #[error("No hook found")]
    Hook(#[from] HookError),

    #[error("Address: {addr:?} is blacklisted")]
    BlacklistedAddress { addr: String },

    #[error("Unexpected game result")]
    UnexpectedGameResult {},

    #[error("Game not found")]
    GameNotFound {},
}
