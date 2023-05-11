mod abi;
mod pb;
use pb::erc20;
use std::str::FromStr;
use substreams::prelude::*;
use substreams::scalar::BigInt;
use substreams::{hex, store::StoreAddBigInt, Hex};
use substreams_ethereum::Event;
use substreams_ethereum::{pb::eth::v2 as eth, NULL_ADDRESS};
use substreams_entity_change::pb::entity::{entity_change::Operation, EntityChanges};
use substreams::store::{DeltaBigInt, Deltas};

// GRT token contract address
const GRT_TOKEN_CONTRACT: [u8; 20] = hex!("c944E90C64B2c07662A292be6244BDf05Cda44a7");

substreams_ethereum::init!();

// -------------------- INITIAL MAPS --------------------

// Initial map for extracting transfers from the blockchain
#[substreams::handlers::map]
fn map_transfers(blk: eth::Block) -> Result<erc20::Transfers, substreams::errors::Error> {
    let mut transfers = erc20::Transfers::default();
    let mut transfers_vec = vec![];

    for log in blk.logs() {
        // Filter logs from GRT token contract only
        if !(&Hex(&GRT_TOKEN_CONTRACT).to_string() == &Hex(&log.address()).to_string()) {
            continue;
        }

        // Create a Transfer proto and add it to transfers if you catch a Transfer event
        if let Some(event) = abi::erc20::events::Transfer::match_and_decode(log) {
            transfers_vec.push(erc20::Transfer {
                id: Hex(&log.receipt.transaction.hash).to_string(), // Each Transfer needs a unique id
                from: Hex(&event.from).to_string(),
                to: Hex(&event.to).to_string(),
                value: event.value.to_string(), // Value is origanally BigInt but proto does not have BigInt so we use string
                ordinal: log.ordinal(),
            })
        }
    }

    transfers.transfers = transfers_vec;
    Ok(transfers)
}

// -------------------- DELTAS MODE STORE --------------------
#[substreams::handlers::store]
fn store_balances(transfers: erc20::Transfers, s: StoreAddBigInt) {
    // For each transfer we extracted in map_events
    for transfer in transfers.transfers {
        // Decrease the balance of the address who sent the transfer
        s.add(
            1,
            &transfer.from,
            BigInt::from_str(&transfer.value).unwrap().neg(),
        );
        // Increase the balance of the address who received the transfer
        s.add(1, &transfer.to, BigInt::from_str(&transfer.value).unwrap());
    }
}
#[substreams::handlers::map]
fn map_accounts(
    grt_deltas: Deltas<DeltaBigInt>,
) -> Result<erc20::Accounts, substreams::errors::Error> {
    let mut accounts = erc20::Accounts::default();
    let mut accounts_vec = vec![];

    for delta in grt_deltas.deltas {
        if delta.key == Hex(&NULL_ADDRESS).to_string() {
            continue;
        }
        accounts_vec.push(erc20::Account {
            id: delta.key, // Each Account needs a unique id
            balance: delta.new_value.to_string(),
        })
    }
    accounts.accounts = accounts_vec;
    Ok(accounts)
}

// -------------------- GET MODE STORE WITH ORDINALS --------------------
#[substreams::handlers::store]
fn store_total_supply(transfers: erc20::Transfers, s: StoreAddBigInt) {
    // For each transfer we extracted in map_events
    for transfer in transfers.transfers {
        if transfer.to == Hex(&NULL_ADDRESS).to_string() {
            s.add(
                transfer.ordinal,
                "totalSupply",
                BigInt::from_str(&transfer.value).unwrap().neg(),
            );
        }
        if transfer.from == Hex(&NULL_ADDRESS).to_string() {
            s.add(transfer.ordinal, "totalSupply", BigInt::from_str(&transfer.value).unwrap());
        }
    }
}

#[substreams::handlers::map]
fn map_block_total_supply_change(
    grt_total_supply: StoreGetBigInt,
) -> Result<erc20::BlockTotalSupplyChange, substreams::errors::Error>  {
    let beginning_of_block_total_supply = match grt_total_supply.get_first("totalSupply"){
        Some(b) => b,
        _ => BigInt::zero(),
    };
    let end_of_block_total_supply = match grt_total_supply.get_last("totalSupply"){
        Some(b) => b,
        _ => BigInt::zero(),
    };
    let block_total_supply_change = end_of_block_total_supply - beginning_of_block_total_supply;

    Ok(erc20::BlockTotalSupplyChange { 
        block_total_supply_change: block_total_supply_change.to_string(),
    })
}


// -------------------- GRAPH_OUT --------------------
// Final module feeding the subgraph with entity changes
#[substreams::handlers::map]
pub fn graph_out(
    transfers: erc20::Transfers,
    grt_balance_deltas: Deltas<DeltaBigInt>,
    grt_total_supply_deltas: Deltas<DeltaBigInt>,
) -> Result<EntityChanges, substreams::errors::Error> {
    let mut entity_changes: EntityChanges = Default::default();
    
    for transfer in transfers.transfers {
        entity_changes
            .push_change(
                "Transfer",
                &transfer.id,
                transfer.ordinal,
                Operation::Update, // Update will create the entity if it does not exist
            )
            .change("to", transfer.to)
            .change("from", transfer.from)
            .change("value", BigInt::from_str(&transfer.value).unwrap());
    }

    for delta in grt_balance_deltas.deltas {
        entity_changes
            .push_change(
                "Account",
                &delta.key, 
                delta.ordinal,
                Operation::Update, // Update will create the entity if it does not exist
            )
            .change("balance", delta);
    }

    for delta in grt_total_supply_deltas.deltas {
        entity_changes
            .push_change(
                "GRT",
                "1",
                delta.ordinal,
                Operation::Update, // Update will create the entity if it does not exist
            )
            .change("totalSupply", delta);
    }

    Ok(EntityChanges {
        entity_changes: 
            entity_changes.entity_changes,
    })
}
