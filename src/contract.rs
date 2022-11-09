#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;

use entropy_beacon_cosmos::beacon::CalculateFeeQuery;
use entropy_beacon_cosmos::EntropyRequest;

use crate::error::ContractError;
use crate::msg::{EntropyCallbackData, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{State, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "entropiclabs/example-entropy-consumer";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Our [`InstantiateMsg`] contains the address of the entropy beacon contract.
/// We save this address in the contract state.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        entropy_beacon_addr: msg.entropy_beacon_addr,
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attribute("method", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // Here we handle requesting entropy from the beacon.
        ExecuteMsg::Coinflip {} => {
            let state = STATE.load(deps.storage)?;
            let beacon_addr = state.entropy_beacon_addr;
            // Note: In production you should check the denomination of the funds to make sure it matches the native token of the chain.
            let sent_amount: Uint128 = info.funds.iter().map(|c| c.amount).sum();

            // How much gas our callback will use. This is an educated guess, so we usually want to overestimate.
            // IF YOU ARE USING THIS CONTRACT AS A TEMPLATE, YOU SHOULD CHANGE THIS VALUE TO MATCH YOUR CONTRACT.
            // If you set this too low, your contract will fail when receiving entropy, and the request will NOT be retried.
            let callback_gas_limit = 100_000u64;

            // The beacon allows us to query the fee it will charge for a request, given the gas limit we provide.
            let beacon_fee =
                CalculateFeeQuery::query(deps.as_ref(), callback_gas_limit, beacon_addr.clone())?;

            // Check if the user sent enough funds to cover the fee.
            if sent_amount < Uint128::from(beacon_fee) {
                return Err(ContractError::InsufficientFunds {});
            }

            Ok(Response::new().add_message(
                EntropyRequest {
                    callback_gas_limit,
                    callback_address: env.contract.address,
                    funds: vec![Coin {
                        denom: "uluna".to_string(), // Change this to match your chain's native token.
                        amount: Uint128::from(beacon_fee),
                    }],
                    // A custom struct and data we define for callback info.
                    // If you are using this contract as a template, you should change this to match the information your contract needs.
                    callback_msg: EntropyCallbackData {
                        original_sender: info.sender,
                    },
                }
                .into_cosmos(beacon_addr)?,
            ))
        }
        // Here we handle receiving entropy from the beacon.
        ExecuteMsg::ReceiveEntropy(data) => {
            let state = STATE.load(deps.storage)?;
            let beacon_addr = state.entropy_beacon_addr;
            // IMPORTANT: Verify that the callback was called by the beacon, and not by someone else.
            if info.sender != beacon_addr {
                return Err(ContractError::Unauthorized {});
            }

            // IMPORTANT: Verify that the original requester for entropy is trusted (e.g.: this contract)
            if data.requester != env.contract.address {
                return Err(ContractError::Unauthorized {});
            }

            // The callback data has 64 bytes of entropy, in a Vec<u8>.
            let entropy = data.entropy;
            // We can parse out our custom callback data from the message.
            let callback_data = data.msg;
            let callback_data = from_binary::<EntropyCallbackData>(&callback_data)?;
            let mut response = Response::new();

            response =
                response.add_attribute("flip_original_caller", callback_data.original_sender);

            // Now we can do whatever we want with the entropy as a randomness source!
            // We can seed a PRNG with the entropy, but here, we just check for even and odd:
            if entropy.last().unwrap() % 2 == 0 {
                response = response.add_attribute("flip_result", "heads");
            } else {
                response = response.add_attribute("flip_result", "tails");
            }
            Ok(response)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!()
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new().add_attribute("action", "migrate"))
}
