// DEX input params without label and ref code

use solana_sdk::pubkey::Pubkey;

use crate::SwapMode;

/// Swap parameters
#[derive(Debug, Clone)]
pub struct SwapParamsIx {
    pub source_mint: Pubkey,
    pub destination_mint: Pubkey,
    pub token_transfer_authority: Pubkey,
    pub amount_in: u64,
    pub swap_mode: SwapMode,
    pub min_out: u64,
    pub salt: [u8; 8],
}

/// Add liquidity parameters
#[derive(Debug, Clone)]
pub struct AddLiquidityParamsIx {
    pub user: Pubkey,
    pub amount_lp: u64, // lp to mint
    pub max_amount_x: u64,
    pub max_amount_y: u64,
}

/// Initialize pool parameters
#[derive(Debug, Clone)]
pub struct InitializePoolParamsIx {
    pub user: Pubkey,
    pub token_x: Pubkey,
    pub token_x_program: Pubkey,
    pub token_y: Pubkey,
    pub token_y_program: Pubkey,
    pub amount_x: u64,
    pub amount_y: u64,
}

/// Remove liquidity parameters
#[derive(Debug, Clone)]
pub struct RemoveLiquidityParamsIx {
    pub user: Pubkey,
    pub amount_lp: u64, // lp to burn
    pub min_amount_x: u64,
    pub min_amount_y: u64,
}

/// Finalize parameters
#[derive(Debug, Clone)]
pub struct FinalizeParamsIx {
    pub settle_signer: Pubkey,
    pub order_owner: Pubkey,
    pub unwrap_wsol: bool,
    pub min_out: u64,
    pub salt: [u8; 8],
    pub output: u64,
    pub commitment: [u8; 32],
    pub deadline: u64,
    pub current_slot: u64,
}
