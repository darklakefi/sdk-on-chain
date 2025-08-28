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
pub mod math;
pub mod darklake_amm;

// Re-export main types for easy access
pub use amm::{Amm, Quote, QuoteParams, SwapParams, SwapMode, SwapAndAccountMetas, DarklakeAmmSwapParams};
pub use darklake_amm::{DarklakeAmm, DARKLAKE_PROGRAM_ID};

/// Create a new Darklake AMM instance from account data
pub fn create_darklake_amm(
    key: solana_sdk::pubkey::Pubkey,
    account_data: &[u8],
) -> anyhow::Result<DarklakeAmm> {
    use anchor_lang::AnchorDeserialize;
    
    let pool = darklake_amm::Pool::deserialize(&mut &account_data[8..])?;
    
    Ok(DarklakeAmm {
        key,
        pool,
        amm_config: darklake_amm::AmmConfig {
            trade_fee_rate: 0,
            create_pool_fee: 0,
            protocol_fee_rate: 0,
            wsol_trade_deposit: 0,
            deadline_slot_duration: 0,
            ratio_change_tolerance_rate: 0,
            bump: 0,
            halted: false,
            padding: [0; 16],
        },
        reserve_x_balance: 0,
        reserve_y_balance: 0,
        token_x_owner: solana_sdk::pubkey::Pubkey::default(),
        token_y_owner: solana_sdk::pubkey::Pubkey::default(),
        token_x_transfer_fee_config: None,
        token_y_transfer_fee_config: None,
    })
}
