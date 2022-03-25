use std::ops::Add;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint64,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::PING_COUNT;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:ping-pong";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    PING_COUNT.save(deps.storage, &Uint64::zero());
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("ping_count", "zero"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let mut count = Uint64::zero();
    match msg {
        ExecuteMsg::Ping {} => {
            let _: Result<Uint64, StdError> = PING_COUNT.update(deps.storage, |exists| {
                count = exists.add(Uint64::from(1u8));
                Ok(count)
            });
            let mut res = Response::new();
            res.attributes.push(attr("ping_count", count));
            res.data = Some(to_binary("pong")?);
            Ok(res)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    let count = PING_COUNT.load(deps.storage)?;
    to_binary(&count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info,
    };
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: Uint64 = from_binary(&res).unwrap();
        assert_eq!(Uint64::zero(), value);
    }

    #[test]
    fn test_ping() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &[]);

        instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::Ping {};
        let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
        assert_eq!(res.attributes.len(), 1);
        assert_eq!(res.attributes, vec![attr("ping_count", "1")]);

        let data: String = from_binary(&res.data.unwrap()).unwrap();
        assert_eq!(data, "pong");

        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes.len(), 1);
        assert_eq!(res.attributes, vec![attr("ping_count", "2")]);

        let data: String = from_binary(&res.data.unwrap()).unwrap();
        assert_eq!(data, "pong");
    }
}
