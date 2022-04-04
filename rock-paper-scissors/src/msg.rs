use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Game, GameMove};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub admin_address: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    StartGame {
        opponent: String,
        first_move: GameMove,
    },
    Respond {
        host: String,
        second_move: GameMove,
    },
    UpdateAdmin {
        admin_address: String,
    },
    AddHook {
        addr: String,
    },
    RemoveHook {
        addr: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetOwner {},
    GetGameByHost { host: String },
    GetGameByOpponent { opponent: String },
    GetAdmin {},
}
