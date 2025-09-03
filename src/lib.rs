//! Darklake DEX SDK
//!
//! A standalone SDK for interacting with Darklake AMM pools.
//! This SDK provides the core functionality for:
//! - Getting quotes for swaps
//! - Building swap instructions
//! - Managing pool state
//!
//! The SDK is designed to be lightweight and focused on Darklake-specific functionality,
//! without the complexity of Jupiter's routing and aggregation features.

pub mod amm;
pub mod darklake_amm;
pub mod math;
pub mod proof;

// Re-export main types for easy access
pub use amm::{
    Amm, DarklakeAmmSettleParams, DarklakeAmmSwapParams, Quote, QuoteParams, SettleAndAccountMetas,
    SettleParams, SwapAndAccountMetas, SwapMode, SwapParams,
};
pub use darklake_amm::{DarklakeAmm, DARKLAKE_PROGRAM_ID};
use solana_sdk::pubkey::Pubkey;

use crate::amm::{AccountData, KeyedAccount};

const POOL_SEED: &[u8] = b"pool";
const AMM_CONFIG_SEED: &[u8] = b"amm_config";

pub fn get_pool_key(token_mint_x: Pubkey, token_mint_y: Pubkey) -> Pubkey {
    // Convert token mints to bytes and ensure x is always below y by lexicographical order
    let (ordered_x, ordered_y) = if token_mint_x < token_mint_y {
        (token_mint_x, token_mint_y)
    } else {
        (token_mint_y, token_mint_x)
    };

    let amm_config_key = Pubkey::find_program_address(
        &[AMM_CONFIG_SEED, &0u32.to_le_bytes()],
        &DARKLAKE_PROGRAM_ID,
    )
    .0;

    Pubkey::find_program_address(
        &[
            POOL_SEED,
            amm_config_key.as_ref(),
            ordered_x.as_ref(),
            ordered_y.as_ref(),
        ],
        &DARKLAKE_PROGRAM_ID,
    )
    .0
}

/// Create a new Darklake AMM instance from account data
pub fn create_darklake_amm(
    pool_key: solana_sdk::pubkey::Pubkey,
    pool_account_data: &[u8],
) -> anyhow::Result<DarklakeAmm> {
    let darklake_amm = DarklakeAmm::from_keyed_account(&KeyedAccount {
        key: pool_key,
        account: AccountData {
            data: pool_account_data.to_vec(),
            owner: DARKLAKE_PROGRAM_ID,
        },
    })?;

    Ok(darklake_amm)
}
