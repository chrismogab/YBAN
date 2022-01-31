use crate::state::{
    can_mint_receipt, increment_num_receipt, read_receipt, store_config, store_receipt, CONFIG,
};
use andromeda_protocol::{
    error::ContractError,
    operators::{execute_update_operators, initialize_operators, query_is_operator},
    ownership::{execute_update_owner, query_contract_owner, CONTRACT_OWNER},
    receipt::{
        Config, ContractInfoResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, Receipt,
        ReceiptResponse,
    },
    require,
};
use cosmwasm_std::{
    attr, entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128,
};
use cw2::{get_contract_version, set_contract_version};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-receipt";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    store_config(deps.storage, &Config { minter: msg.minter })?;
    if let Some(operators) = msg.operators {
        initialize_operators(deps.storage, operators)?;
    }
    CONTRACT_OWNER.save(deps.storage, &info.sender)?;
    Ok(Response::default()
        .add_attributes(vec![attr("action", "instantiate"), attr("type", "receipt")]))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::StoreReceipt { receipt } => execute_store_receipt(deps, info, receipt),
        ExecuteMsg::EditReceipt {
            receipt,
            receipt_id,
        } => execute_edit_receipt(deps, info, receipt_id, receipt),
        ExecuteMsg::UpdateOwner { address } => execute_update_owner(deps, info, address),
        ExecuteMsg::UpdateOperator { operators } => execute_update_operators(deps, info, operators),
    }
}

fn execute_store_receipt(
    deps: DepsMut,
    info: MessageInfo,
    receipt: Receipt,
) -> Result<Response, ContractError> {
    require(
        can_mint_receipt(deps.storage, &info.sender.to_string())?,
        ContractError::Unauthorized {},
    )?;
    let receipt_id = increment_num_receipt(deps.storage)?;
    store_receipt(deps.storage, receipt_id, &receipt)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "mint_receipt"),
        attr("receipt_id", receipt_id.to_string()),
    ]))
}

fn execute_edit_receipt(
    deps: DepsMut,
    info: MessageInfo,
    receipt_id: Uint128,
    receipt: Receipt,
) -> Result<Response, ContractError> {
    require(
        can_mint_receipt(deps.storage, &info.sender.to_string())?,
        ContractError::Unauthorized {},
    )?;
    read_receipt(deps.storage, receipt_id)?;
    store_receipt(deps.storage, receipt_id, &receipt)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "edit_receipt"),
        attr("receipt_id", receipt_id.to_string()),
        attr("receipt_edited_by", info.sender.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let version = get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(ContractError::CannotMigrate {
            previous_contract: version.contract,
        });
    }
    Ok(Response::default())
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Receipt { receipt_id } => to_binary(&query_receipt(deps, receipt_id)?),
        QueryMsg::ContractInfo {} => to_binary(&query_config(deps)?),
        QueryMsg::ContractOwner {} => to_binary(&query_contract_owner(deps)?),
        QueryMsg::IsOperator { address } => to_binary(&query_is_operator(deps, &address)?),
    }
}

fn query_receipt(deps: Deps, receipt_id: Uint128) -> StdResult<ReceiptResponse> {
    let receipt = read_receipt(deps.storage, receipt_id)?;
    Ok(ReceiptResponse { receipt })
}

fn query_config(deps: Deps) -> StdResult<ContractInfoResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ContractInfoResponse { config })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{
        from_binary,
        testing::{mock_dependencies, mock_env, mock_info},
        Addr, Event,
    };
    #[test]
    fn test_instantiate() {
        let owner = "creator";
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info(owner, &[]);
        let msg = InstantiateMsg {
            minter: owner.to_string(),
            operators: None,
        };
        let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn test_store_receipt() {
        let owner = "creator";
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info(owner, &[]);
        let unauth_info = mock_info("anyone", &[]);
        let config = Config {
            minter: owner.to_string(),
        };
        store_config(deps.as_mut().storage, &config).unwrap();
        CONTRACT_OWNER
            .save(deps.as_mut().storage, &Addr::unchecked(owner.to_string()))
            .unwrap();

        let msg = ExecuteMsg::StoreReceipt {
            receipt: Receipt { events: vec![] },
        };

        let res_unauth = execute(deps.as_mut(), env.clone(), unauth_info, msg.clone()).unwrap_err();
        assert_eq!(res_unauth, ContractError::Unauthorized {});

        //add address for registered operator
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(
            Response::new().add_attributes(vec![
                attr("action", "mint_receipt"),
                attr("receipt_id", "1"),
            ]),
            res
        );
    }

    #[test]
    fn test_edit_receipt() {
        let owner = "creator";
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info(owner, &[]);
        let unauth_info = mock_info("anyone", &[]);
        let config = Config {
            minter: owner.to_string(),
        };

        CONTRACT_OWNER
            .save(deps.as_mut().storage, &Addr::unchecked(owner.to_string()))
            .unwrap();

        store_config(deps.as_mut().storage, &config).unwrap();

        let store_msg = ExecuteMsg::StoreReceipt {
            receipt: Receipt {
                events: vec![Event::new("test")],
            },
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), store_msg).unwrap();
        assert_eq!(
            Response::new().add_attributes(vec![
                attr("action", "mint_receipt"),
                attr("receipt_id", "1"),
            ]),
            res
        );

        let new_receipt = Receipt {
            events: vec![Event::new("new")],
        };
        let msg = ExecuteMsg::EditReceipt {
            receipt_id: Uint128::from(1_u128),
            receipt: new_receipt.clone(),
        };

        let res_unauth = execute(deps.as_mut(), env.clone(), unauth_info, msg.clone()).unwrap_err();
        assert_eq!(res_unauth, ContractError::Unauthorized {});

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        let expected = Response::default().add_attributes(vec![
            attr("action", "edit_receipt"),
            attr("receipt_id", "1"),
            attr("receipt_edited_by", info.sender.to_string()),
        ]);

        assert_eq!(res, expected);

        let query_msg = QueryMsg::Receipt {
            receipt_id: Uint128::from(1_u128),
        };
        let res = query(deps.as_ref(), env, query_msg).unwrap();
        let val: ReceiptResponse = from_binary(&res).unwrap();

        assert_eq!(val.receipt, new_receipt)
    }
}
