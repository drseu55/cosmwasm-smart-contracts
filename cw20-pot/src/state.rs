use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, DepsMut, StdResult, Uint128, Uint64};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub cw20_addr: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Pot {
    pub target_addr: Addr,
    pub threshold_amount: Uint128,
    pub collected: Uint128,
}

pub fn save_pot(deps: DepsMut, pot: &Pot) -> StdResult<()> {
    // increment id if exists, or return 1
    let id = POT_SEQ.load(deps.storage)?;
    // checks for overflow
    let id = id.checked_add(Uint64::new(1))?;
    POT_SEQ.save(deps.storage, &id)?;

    POTS.save(deps.storage, id.u64().into(), pot)
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const POT_SEQ: Item<Uint64> = Item::new("pot_seq");
pub const POTS: Map<u64, Pot> = Map::new("pot");
