use cosmwasm_std::{Coin, StdError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Cannot create expired option")]
    CreateExpired {},

    #[error("Option expired")]
    Expired {},

    #[error("Option not expired")]
    NotExpired {},

    #[error("Must send exact counter_offer: {val:?}")]
    NotEqualCounterOffer { val: Vec<Coin> },

    #[error("Don't send funds when burn")]
    BurnFunds {},

    #[error("Custom Error val: {val:?}")]
    CustomError { val: String },
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
