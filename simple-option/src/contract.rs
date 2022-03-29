#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{State, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:simple-option";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    if msg.expires <= env.block.height {
        return Err(ContractError::CreateExpired {});
    }

    let state = State {
        creator: info.sender.clone(),
        owner: info.sender.clone(),
        collateral: info.funds,
        counter_offer: msg.counter_offer,
        expires: msg.expires,
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("creator", info.sender.clone())
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer { recipient } => handle_transfer(deps, info, recipient),
        ExecuteMsg::Execute {} => handle_execute(deps, env, info),
        ExecuteMsg::Burn {} => handle_burn(deps, env, info),
    }
}

pub fn handle_transfer(
    deps: DepsMut,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let state_loaded = STATE.load(deps.storage)?;

    if info.sender != state_loaded.owner {
        return Err(ContractError::Unauthorized {});
    }

    let recipient_address = deps.api.addr_validate(&recipient)?;

    // set new owner on state
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.owner = recipient_address;
        Ok(state)
    })?;

    Ok(Response::new().add_attribute("method", "handle_transfer"))
}

pub fn handle_execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;

    if info.sender != state.owner {
        return Err(ContractError::Unauthorized {});
    }

    // ensure not expired
    if env.block.height >= state.expires {
        return Err(ContractError::Expired {});
    }

    // ensure sending proper counter_offer
    if info.funds != state.counter_offer {
        return Err(ContractError::NotEqualCounterOffer {
            val: state.counter_offer,
        });
    }

    // delete the option
    STATE.remove(deps.storage);

    let res = Response::new()
        .add_message(BankMsg::Send {
            to_address: state.creator.to_string(),
            amount: state.counter_offer,
        })
        .add_message(BankMsg::Send {
            to_address: state.owner.to_string(),
            amount: state.collateral,
        })
        .add_attribute("method", "handle_execute");
    Ok(res)
}

pub fn handle_burn(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;

    // ensure is expired
    if env.block.height < state.expires {
        return Err(ContractError::NotExpired {});
    }

    // ensure sending proper counter_offer
    if !info.funds.is_empty() {
        return Err(ContractError::BurnFunds {});
    }

    // delete the option
    STATE.remove(deps.storage);

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: state.creator.to_string(),
            amount: state.collateral,
        })
        .add_attribute("method", "handle_burn"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info,
    };
    use cosmwasm_std::{attr, coins, from_binary, CosmosMsg, ReplyOn, SubMsg};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            counter_offer: coins(40, "ETH"),
            expires: 100_000,
        };
        let info = mock_info("creator", &coins(1, "BTC"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(100_000, value.expires);
        assert_eq!("creator", value.owner);
        assert_eq!("creator", value.creator);
        assert_eq!(coins(1, "BTC"), value.collateral);
        assert_eq!(coins(40, "ETH"), value.counter_offer);
    }

    #[test]
    fn transfer() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            counter_offer: coins(40, "ETH"),
            expires: 100_000,
        };
        let info = mock_info("creator", &coins(1, "BTC"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // random cannot transfer
        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::Transfer {
            recipient: "Someone".to_string(),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg);

        match res.unwrap_err() {
            ContractError::Unauthorized {} => {}
            e => panic!("Unexpected error: {:?}", e),
        }

        // owner can transfer
        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::Transfer {
            recipient: "Someone".to_string(),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes.len(), 1);
        assert_eq!(res.attributes[0], attr("method", "handle_transfer"));

        // check updated properly
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(100_000, value.expires);
        assert_eq!("Someone", value.owner);
        assert_eq!("creator", value.creator);
    }

    #[test]
    fn execute_test() {
        let mut deps = mock_dependencies();

        let counter_offer = coins(40, "ETH");
        let msg = InstantiateMsg {
            counter_offer: counter_offer.clone(),
            expires: 100_000,
        };
        let info = mock_info("creator", &coins(1, "BTC"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // random cannot execute
        let info = mock_info("anyone", &counter_offer);
        let msg = ExecuteMsg::Execute {};
        let res = execute(deps.as_mut(), mock_env(), info, msg);

        match res.unwrap_err() {
            ContractError::Unauthorized {} => {}
            e => panic!("Unexpected error: {:?}", e),
        }

        // expired cannot execute
        let mut info = mock_info("creator", &counter_offer);
        let mut env = mock_env();
        env.block.height = 200_000;
        let msg = ExecuteMsg::Execute {};
        let res = execute(deps.as_mut(), env, info, msg.clone());

        match res.unwrap_err() {
            ContractError::Expired {} => {}
            e => panic!("Unexpected error: {:?}", e),
        }

        // bad counter_offer cannot execute
        let msg_offer = coins(39, "ETH");
        let info = mock_info("creator", &msg_offer);
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        match res.unwrap_err() {
            ContractError::NotEqualCounterOffer { val } => {
                assert_eq!(val, counter_offer)
            }
            e => panic!("unexpected error: {}", e),
        }

        // proper execution
        let info = mock_info("creator", &counter_offer);
        let msg = ExecuteMsg::Execute {};
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.messages.len(), 2);
        assert_eq!(
            res.messages[0],
            SubMsg {
                id: 0,
                msg: BankMsg::Send {
                    to_address: "creator".into(),
                    amount: counter_offer,
                }
                .into(),
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );
        assert_eq!(
            res.messages[1],
            SubMsg {
                id: 0,
                msg: BankMsg::Send {
                    to_address: "creator".into(),
                    amount: coins(1, "BTC"),
                }
                .into(),
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );

        // check updated properly
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap_err();
    }

    #[test]
    fn burn() {
        let mut deps = mock_dependencies();

        let counter_offer = coins(40, "ETH");
        let collateral = coins(1, "BTC");
        let msg_expires = 100_000;
        let msg = InstantiateMsg {
            counter_offer: counter_offer.clone(),
            expires: msg_expires,
        };
        let info = mock_info("creator", &collateral);

        // we can just call .unwrap() to assert this was a success
        let _ = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // set new owner
        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::Transfer {
            recipient: "owner".to_string(),
        };
        let _ = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // non-expired cannot execute
        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::Burn {};
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        match err {
            ContractError::NotExpired {} => {}
            e => panic!("unexpected error: {}", e),
        }

        // with funds cannot execute
        let info = mock_info("anyone", &counter_offer);
        let mut env = mock_env();
        env.block.height = 200_000;
        let msg = ExecuteMsg::Burn {};
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        match err {
            ContractError::BurnFunds {} => {}
            e => panic!("unexpected error: {}", e),
        }

        // expired returns funds
        let info = mock_info("anyone", &[]);
        let mut env = mock_env();
        env.block.height = 200_000;
        let msg = ExecuteMsg::Burn {};
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.messages.len(), 1);
        assert_eq!(
            res.messages[0],
            SubMsg {
                id: 0,
                msg: BankMsg::Send {
                    to_address: "creator".into(),
                    amount: collateral,
                }
                .into(),
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );

        // check deleted
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap_err();
    }
}
