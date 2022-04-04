#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response, StdError, StdResult,
};
use cw2::set_contract_version;
use cw_storage_plus::Bound;
use cw_utils::maybe_addr;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Game, GameMove, GameResult, State, ADMIN, GAME, HOOKS, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:rock-paper-scissors";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        owner: info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    let deps_api = deps.api;

    ADMIN.set(deps.branch(), maybe_addr(deps_api, msg.admin_address)?)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let deps_api = deps.api;

    match msg {
        ExecuteMsg::StartGame {
            opponent,
            first_move,
        } => execute_start_game(deps, info, opponent, first_move),
        ExecuteMsg::Respond { host, second_move } => execute_respond(deps, info, host, second_move),
        ExecuteMsg::UpdateAdmin { admin_address } => Ok(ADMIN.execute_update_admin(
            deps,
            info,
            maybe_addr(deps_api.clone(), Some(admin_address))?,
        )?),
        ExecuteMsg::AddHook { addr } => Ok(HOOKS.execute_add_hook(
            &ADMIN,
            deps,
            info,
            deps_api.clone().addr_validate(&addr)?,
        )?),
        ExecuteMsg::RemoveHook { addr } => Ok(HOOKS.execute_remove_hook(
            &ADMIN,
            deps,
            info,
            deps_api.clone().addr_validate(&addr)?,
        )?),
    }
}

pub fn execute_start_game(
    deps: DepsMut,
    info: MessageInfo,
    opponent: String,
    first_move: GameMove,
) -> Result<Response, ContractError> {
    let validated_opponent_address = deps.api.addr_validate(&opponent)?;

    let hooks_response = HOOKS.query_hooks(deps.as_ref())?;

    if hooks_response
        .hooks
        .contains(&info.sender.clone().to_string())
    {
        return Err(ContractError::BlacklistedAddress {
            addr: info.sender.clone().to_string(),
        });
    }

    let game = Game {
        host: info.sender.clone(),
        opponent: validated_opponent_address.clone(),
        host_move: first_move,
        opp_move: None,
        result: None,
    };

    GAME.save(
        deps.storage,
        (&info.sender, &validated_opponent_address),
        &game,
    )?;

    Ok(Response::new().add_attribute("method", "execute_start_game"))
}

pub fn get_result(game: Game) -> Result<GameResult, ContractError> {
    let opponent_move = game
        .opp_move
        .ok_or(ContractError::UnexpectedGameResult {})?;

    match game.host_move {
        GameMove::Paper => match opponent_move {
            GameMove::Paper => Ok(GameResult::Tie),
            GameMove::Rock => Ok(GameResult::HostWins),
            GameMove::Scissors => Ok(GameResult::OpponentWins),
        },
        GameMove::Rock => match opponent_move {
            GameMove::Paper => Ok(GameResult::OpponentWins),
            GameMove::Rock => Ok(GameResult::Tie),
            GameMove::Scissors => Ok(GameResult::HostWins),
        },
        GameMove::Scissors => match opponent_move {
            GameMove::Paper => Ok(GameResult::HostWins),
            GameMove::Rock => Ok(GameResult::OpponentWins),
            GameMove::Scissors => Ok(GameResult::Tie),
        },
    }
}

pub fn execute_respond(
    deps: DepsMut,
    info: MessageInfo,
    host: String,
    second_move: GameMove,
) -> Result<Response, ContractError> {
    let host_address = deps.api.addr_validate(&host)?;

    let mut game_load = match GAME.load(deps.storage, (&host_address, &info.sender)) {
        Ok(game) => game,
        _ => return Err(ContractError::GameNotFound {}),
    };

    game_load.opp_move = Some(second_move.clone());
    let game_result_tmp = Some(get_result(game_load)?);

    let game = GAME.update(
        deps.storage,
        (&host_address, &info.sender),
        |state| -> Result<_, ContractError> {
            match state {
                Some(mut game) => {
                    game.opp_move = Some(second_move);
                    game.result = game_result_tmp;

                    Ok(game)
                }
                None => Err(ContractError::GameNotFound {}),
            }
        },
    )?;

    GAME.remove(deps.storage, (&host_address, &info.sender));

    let game_result = match game.result {
        Some(GameResult::HostWins) => "Host Wins".to_string(),
        Some(GameResult::OpponentWins) => "Opponent Wins".to_string(),
        Some(GameResult::Tie) => "Tie".to_string(),
        _ => panic!("Unexpected result"),
    };

    Ok(Response::new()
        .add_attribute("method", "execute_respond")
        .add_attribute("result", game_result))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetOwner {} => to_binary(&get_owner(deps)?),
        QueryMsg::GetGameByHost { host } => to_binary(&get_game_by_host(deps, host)?),
        QueryMsg::GetGameByOpponent { opponent } => {
            to_binary(&get_game_by_opponent(deps, opponent)?)
        }
        QueryMsg::GetAdmin {} => to_binary(&ADMIN.query_admin(deps)?),
    }
}

fn get_owner(deps: Deps) -> StdResult<String> {
    let state = STATE.load(deps.storage)?;
    Ok(state.owner.to_string())
}

fn get_game_by_host(deps: Deps, host: String) -> StdResult<Vec<Game>> {
    let validated_host = &deps.api.addr_validate(&host)?;

    let mut host_games: Vec<Game> = Vec::new();

    let host_games_result: StdResult<Vec<(Addr, Game)>> = GAME
        .prefix(validated_host)
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    for game in host_games_result? {
        host_games.push(game.1);
    }

    Ok(host_games)
}

fn get_game_by_opponent(deps: Deps, opponent: String) -> StdResult<Vec<Game>> {
    let validated_opponent = &deps.api.addr_validate(&opponent)?;

    let mut opponent_games: Vec<Game> = Vec::new();

    let all_games: StdResult<Vec<((Addr, Addr), Game)>> = GAME
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    for game in all_games? {
        if validated_opponent == &game.1.opponent {
            opponent_games.push(game.1);
        }
    }

    Ok(opponent_games)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info,
    };
    use cosmwasm_std::{coins, from_binary, Addr, Api};
    use cw_controllers::AdminResponse;

    #[test]
    fn proper_initialization_without_admin() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin_address: None,
        };

        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetOwner {}).unwrap();
        let value: String = from_binary(&res).unwrap();
        assert_eq!(String::from("creator"), value);

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetAdmin {}).unwrap();
        let value: AdminResponse = from_binary(&res).unwrap();
        assert_eq!(None, value.admin);
    }

    #[test]
    fn proper_initialization_with_admin() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin_address: Some("admin".to_string()),
        };

        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetOwner {}).unwrap();
        let value: String = from_binary(&res).unwrap();
        assert_eq!(String::from("creator"), value);

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetAdmin {}).unwrap();
        let value: AdminResponse = from_binary(&res).unwrap();
        assert_eq!(Some("admin".to_string()), value.admin);
    }

    #[test]
    fn test_start_game() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin_address: None,
        };

        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // try with invalid address
        let opponent = String::from("11111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111");
        let msg = ExecuteMsg::StartGame {
            opponent,
            first_move: GameMove::Paper,
        };
        let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone());
        match res {
            e => println!("Error: {:?}", e),
        }

        // start game`
        let opponent = String::from("someone_different");
        let msg = ExecuteMsg::StartGame {
            opponent,
            first_move: GameMove::Paper,
        };
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    }

    #[test]
    fn test_query_host_games() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin_address: None,
        };

        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // start game - host is `creator`, opponent is `someone_different`
        let opponent = String::from("someone_different");
        let msg = ExecuteMsg::StartGame {
            opponent,
            first_move: GameMove::Paper,
        };
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // start another game - host is `creator`, opponent is `someone_different2`
        let opponent = String::from("someone_different2");
        let msg = ExecuteMsg::StartGame {
            opponent,
            first_move: GameMove::Paper,
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // query `creator` games
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetGameByHost {
                host: "creator".to_string(),
            },
        )
        .unwrap();
        let value: Vec<Game> = from_binary(&res).unwrap();
        assert_eq!(
            vec![
                Game {
                    host: Addr::unchecked("creator"),
                    opponent: Addr::unchecked("someone_different"),
                    host_move: GameMove::Paper,
                    opp_move: None,
                    result: None,
                },
                Game {
                    host: Addr::unchecked("creator"),
                    opponent: Addr::unchecked("someone_different2"),
                    host_move: GameMove::Paper,
                    opp_move: None,
                    result: None,
                }
            ],
            value
        );
    }

    #[test]
    fn test_query_opponent_games() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin_address: None,
        };

        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // start game - host is `creator`, opponent is `someone_different`
        let opponent = String::from("someone_different");
        let msg = ExecuteMsg::StartGame {
            opponent,
            first_move: GameMove::Paper,
        };
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // start game2 - host is `creator`, opponent is `someone_different`
        let opponent = String::from("someone_different");
        let info = mock_info("creator2", &[]);
        let msg = ExecuteMsg::StartGame {
            opponent,
            first_move: GameMove::Paper,
        };
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // start game3 - host is `creator`, opponent is `someone_different2`
        let opponent = String::from("someone_different2");
        let msg = ExecuteMsg::StartGame {
            opponent,
            first_move: GameMove::Paper,
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // query `creator` games
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetGameByOpponent {
                opponent: "someone_different".to_string(),
            },
        )
        .unwrap();
        let value: Vec<Game> = from_binary(&res).unwrap();
        assert_eq!(
            vec![
                Game {
                    host: Addr::unchecked("creator"),
                    opponent: Addr::unchecked("someone_different"),
                    host_move: GameMove::Paper,
                    opp_move: None,
                    result: None,
                },
                Game {
                    host: Addr::unchecked("creator2"),
                    opponent: Addr::unchecked("someone_different"),
                    host_move: GameMove::Paper,
                    opp_move: None,
                    result: None,
                }
            ],
            value
        );
    }

    #[test]
    fn test_blacklisting() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin_address: Some("creator".to_string()),
        };

        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // blacklist an address
        let msg = ExecuteMsg::AddHook {
            addr: "elona_musk".to_string(),
        };
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // match error response when starting a game, because address is blacklisted
        let info = mock_info("elona_musk", &[]);
        let msg = ExecuteMsg::StartGame {
            opponent: "someone".to_string(),
            first_move: GameMove::Paper,
        };
        let res = execute(deps.as_mut(), mock_env(), info.clone(), msg);
        match res.unwrap_err() {
            ContractError::BlacklistedAddress { addr } => {}
            _ => panic!("Unexpected error"),
        }

        // TODO: Add test for removing address from blacklist
    }

    #[test]
    fn respond_to_someone_else_game() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin_address: None,
        };

        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // start game`
        let opponent = String::from("someone");
        let msg = ExecuteMsg::StartGame {
            opponent,
            first_move: GameMove::Paper,
        };
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // info.sender is different from opponent
        let info = mock_info("someone_else", &[]);
        let msg = ExecuteMsg::Respond {
            host: "creator".to_string(),
            second_move: GameMove::Paper,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        match res {
            ContractError::GameNotFound {} => {}
            e => panic!("Unexpected Error: {:?}", e),
        }
    }

    #[test]
    fn host_wins() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin_address: None,
        };

        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // start game`
        let opponent = String::from("someone");
        let msg = ExecuteMsg::StartGame {
            opponent,
            first_move: GameMove::Paper,
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // check if game exists
        let msg = QueryMsg::GetGameByHost {
            host: "creator".to_string(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: Vec<Game> = from_binary(&res).unwrap();
        assert_eq!(
            vec![Game {
                host: Addr::unchecked("creator"),
                opponent: Addr::unchecked("someone"),
                host_move: GameMove::Paper,
                opp_move: None,
                result: None,
            }],
            value
        );

        // someone responds with rock and result should be HostWins
        let info = mock_info("someone", &[]);
        let msg = ExecuteMsg::Respond {
            host: String::from("creator"),
            second_move: GameMove::Rock,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(res.attributes[1].value, String::from("Host Wins"));

        // check if game is deleted
        let msg = QueryMsg::GetGameByHost {
            host: "creator".to_string(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: Vec<Game> = from_binary(&res).unwrap();
        let empty_vec: Vec<Game> = Vec::new();
        assert_eq!(empty_vec, value);
    }

    #[test]
    fn opponent_wins() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin_address: None,
        };

        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // start game`
        let opponent = String::from("someone");
        let msg = ExecuteMsg::StartGame {
            opponent,
            first_move: GameMove::Paper,
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // check if game exists
        let msg = QueryMsg::GetGameByHost {
            host: "creator".to_string(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: Vec<Game> = from_binary(&res).unwrap();
        assert_eq!(
            vec![Game {
                host: Addr::unchecked("creator"),
                opponent: Addr::unchecked("someone"),
                host_move: GameMove::Paper,
                opp_move: None,
                result: None,
            }],
            value
        );

        // someone responds with rock and result should be HostWins
        let info = mock_info("someone", &[]);
        let msg = ExecuteMsg::Respond {
            host: String::from("creator"),
            second_move: GameMove::Scissors,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(res.attributes[1].value, String::from("Opponent Wins"));

        // check if game is deleted
        let msg = QueryMsg::GetGameByHost {
            host: "creator".to_string(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: Vec<Game> = from_binary(&res).unwrap();
        let empty_vec: Vec<Game> = Vec::new();
        assert_eq!(empty_vec, value);
    }

    #[test]
    fn tie() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            admin_address: None,
        };

        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // start game`
        let opponent = String::from("someone");
        let msg = ExecuteMsg::StartGame {
            opponent,
            first_move: GameMove::Paper,
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // check if game exists
        let msg = QueryMsg::GetGameByHost {
            host: "creator".to_string(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: Vec<Game> = from_binary(&res).unwrap();
        assert_eq!(
            vec![Game {
                host: Addr::unchecked("creator"),
                opponent: Addr::unchecked("someone"),
                host_move: GameMove::Paper,
                opp_move: None,
                result: None,
            }],
            value
        );

        // someone responds with rock and result should be HostWins
        let info = mock_info("someone", &[]);
        let msg = ExecuteMsg::Respond {
            host: String::from("creator"),
            second_move: GameMove::Paper,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(res.attributes[1].value, String::from("Tie"));

        // check if game is deleted
        let msg = QueryMsg::GetGameByHost {
            host: "creator".to_string(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: Vec<Game> = from_binary(&res).unwrap();
        let empty_vec: Vec<Game> = Vec::new();
        assert_eq!(empty_vec, value);
    }
}
