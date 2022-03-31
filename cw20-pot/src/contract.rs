#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128, Uint64,
};
use cw2::set_contract_version;
use cw20::{Cw20Contract, Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::error::ContractError;
use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, PotResponse, QueryMsg, ReceiveMsg};
use crate::state::{save_pot, Config, Pot, CONFIG, POTS, POT_SEQ};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-pot";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let owner = msg
        .admin
        .and_then(|s| deps.api.addr_validate(s.as_str()).ok())
        .unwrap_or(info.sender);

    let config = Config {
        owner: owner.clone(),
        cw20_addr: deps.api.addr_validate(msg.cw20_addr.as_str())?,
    };

    CONFIG.save(deps.storage, &config)?;

    POT_SEQ.save(deps.storage, &Uint64::new(0))?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", owner)
        .add_attribute("cw20_addr", msg.cw20_addr))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreatePot {
            target_addr,
            threshold,
        } => execute_create_pot(deps, info, target_addr, threshold),
        ExecuteMsg::Receive(msg) => execute_receive(deps, info, msg),
    }
}

pub fn execute_create_pot(
    deps: DepsMut,
    info: MessageInfo,
    target_addr: String,
    threshold: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // create and save pot
    let pot = Pot {
        target_addr: deps.api.addr_validate(target_addr.as_str())?,
        threshold_amount: threshold,
        collected: Uint128::zero(),
    };
    save_pot(deps, &pot)?;

    Ok(Response::new()
        .add_attribute("action", "execute_create_pot")
        .add_attribute("target_addr", target_addr)
        .add_attribute("threshold_amount", threshold))
}

pub fn execute_receive(
    deps: DepsMut,
    info: MessageInfo,
    wrapped: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.cw20_addr != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let msg: ReceiveMsg = from_binary(&wrapped.msg)?;
    match msg {
        ReceiveMsg::Send { id } => receive_send(deps, id, wrapped.amount, info.sender),
    }
}

pub fn receive_send(
    deps: DepsMut,
    pot_id: Uint64,
    amount: Uint128,
    cw20_addr: Addr,
) -> Result<Response, ContractError> {
    let mut pot = POTS.load(deps.storage, pot_id.u64().into())?;

    pot.collected += amount;

    POTS.save(deps.storage, pot_id.u64().into(), &pot)?;

    let mut res = Response::new()
        .add_attribute("action", "receive_send")
        .add_attribute("pot_id", pot_id)
        .add_attribute("collected", pot.collected)
        .add_attribute("threshold", pot.threshold_amount);

    // if collected exceeds threshold prepare a cw20 message
    if pot.collected >= pot.threshold_amount {
        let cw20 = Cw20Contract(cw20_addr);
        // Build a cw20 transfer send msg, that send collected funds to target address
        let msg = cw20.call(Cw20ExecuteMsg::Transfer {
            recipient: pot.target_addr.into_string(),
            amount: pot.collected,
        })?;
        res = res.add_message(msg);
    }

    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_binary(&query_config(deps)?),
        QueryMsg::GetPot { id } => to_binary(&query_pot(deps, id)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner,
        cw20_addr: config.cw20_addr,
    })
}

fn query_pot(deps: Deps, id: Uint64) -> StdResult<PotResponse> {
    let pot = POTS.load(deps.storage, id.u64().into())?;
    Ok(PotResponse {
        target_addr: pot.target_addr.into_string(),
        collected: pot.collected,
        threshold: pot.threshold_amount,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info, MOCK_CONTRACT_ADDR,
    };
    use cosmwasm_std::{coins, from_binary, CosmosMsg, WasmMsg};

    #[test]
    fn proper_initialization_without_admin() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: None,
            cw20_addr: "someone".to_string(),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetConfig {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("creator", value.owner.as_str());
        assert_eq!("someone", value.cw20_addr.as_str());
    }

    #[test]
    fn proper_initialization_with_admin() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: Some("admin_addr".to_string()),
            cw20_addr: "someone".to_string(),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetConfig {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("admin_addr", value.owner.as_str());
        assert_eq!("someone", value.cw20_addr.as_str());
    }

    #[test]
    fn create_pot() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: None,
            cw20_addr: String::from(MOCK_CONTRACT_ADDR),
        };

        let info = mock_info("creator", &[]);

        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // should create pot
        let msg = ExecuteMsg::CreatePot {
            target_addr: String::from("Some"),
            threshold: Uint128::new(100),
        };

        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.messages.len(), 0);

        // query pot
        let msg = QueryMsg::GetPot { id: Uint64::new(1) };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();

        let pot: PotResponse = from_binary(&res).unwrap();
        assert_eq!(
            pot,
            PotResponse {
                target_addr: Addr::unchecked("Some").to_string(),
                threshold: Uint128::new(100),
                collected: Default::default(),
            }
        );
    }

    #[test]
    fn test_receive_send() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin: None,
            cw20_addr: String::from("cw20"),
        };
        let mut info = mock_info("creator", &[]);

        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // should create pot
        let msg = ExecuteMsg::CreatePot {
            target_addr: String::from("Some"),
            threshold: Uint128::new(100),
        };

        let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(res.messages.len(), 0);

        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: String::from("cw20"),
            amount: Uint128::new(55),
            msg: to_binary(&ReceiveMsg::Send { id: Uint64::new(1) }).unwrap(),
        });
        info.sender = Addr::unchecked("cw20");
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // query pot
        let msg = QueryMsg::GetPot { id: Uint64::new(1) };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();

        let pot: PotResponse = from_binary(&res).unwrap();
        assert_eq!(
            pot,
            PotResponse {
                target_addr: Addr::unchecked("Some").to_string(),
                threshold: Uint128::new(100),
                collected: Uint128::new(55),
            }
        );

        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: String::from("cw20"),
            amount: Uint128::new(55),
            msg: to_binary(&ReceiveMsg::Send { id: Uint64::new(1) }).unwrap(),
        });

        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        let msg = res.messages[0].clone().msg;
        assert_eq!(
            msg,
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: String::from("cw20"),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: String::from("Some"),
                    amount: Uint128::new(110)
                })
                .unwrap(),
                funds: vec![]
            })
        );

        // query pot
        let msg = QueryMsg::GetPot { id: Uint64::new(1) };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();

        let pot: PotResponse = from_binary(&res).unwrap();
        assert_eq!(
            pot,
            PotResponse {
                target_addr: Addr::unchecked("Some").to_string(),
                threshold: Uint128::new(100),
                collected: Uint128::new(110),
            }
        );
    }
}
