use anchor_lang::prelude::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use anyhow::Result;

/// Core AMM trait for Darklake DEX operations
pub trait Amm: Send + Sync {
    /// Get the label/name of the AMM
    fn label(&self) -> String;
    
    /// Deserialize the AMM from a keyed account
    fn from_keyed_account(keyed_account: &KeyedAccount) -> Result<Self>
    where
        Self: Sized;

    /// Get the program ID of the AMM
    fn program_id(&self) -> Pubkey;
    
    /// Get the key/address of the AMM
    fn key(&self) -> Pubkey;
    
    /// Get the reserve token mints
    fn get_reserve_mints(&self) -> Vec<Pubkey>;
    
    /// Get accounts that need to be updated
    fn get_accounts_to_update(&self) -> Vec<Pubkey>;
    
    /// Update the AMM state from account data
    fn update(&mut self, account_map: &AccountMap) -> Result<()>;
    
    /// Check if exact out swaps are supported
    fn supports_exact_out(&self) -> bool;
    
    /// Get a quote for a swap
    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote>;
    
    /// Get swap parameters and account metadata
    fn get_swap_and_account_metas(&self, swap_params: &SwapParams) -> Result<SwapAndAccountMetas>;
    
    /// Clone the AMM
    fn clone_amm(&self) -> Box<dyn Amm + Send + Sync>;
    
    /// Check if the AMM is active
    fn is_active(&self) -> bool;
}

/// Account map for storing account data
pub type AccountMap = std::collections::HashMap<Pubkey, AccountData>;

/// Account data structure
#[derive(Clone, Debug)]
pub struct AccountData {
    pub data: Vec<u8>,
    pub owner: Pubkey,
}

/// Quote parameters for swap operations
#[derive(Debug, Clone)]
pub struct QuoteParams {
    pub input_mint: Pubkey,
    pub amount: u64,
    pub swap_mode: SwapMode,
}

/// Swap mode (exact in/out)
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum SwapMode {
    ExactIn,
    ExactOut,
}

/// Quote result
#[derive(Debug, Clone)]
pub struct Quote {
    pub in_amount: u64,
    pub out_amount: u64,
    pub fee_amount: u64,
    pub fee_mint: Pubkey,
    pub fee_pct: rust_decimal::Decimal,
}

/// Swap parameters
#[derive(Debug, Clone)]
pub struct SwapParams {
    pub source_mint: Pubkey,
    pub destination_mint: Pubkey,
    pub source_token_account: Pubkey,
    pub destination_token_account: Pubkey,
    pub token_transfer_authority: Pubkey,
    pub in_amount: u64,
    pub minimum_out_amount: u64,
    pub swap_mode: SwapMode,
}

/// Swap result with account metadata
#[derive(Debug, Clone)]
pub struct SwapAndAccountMetas {
    pub swap: DarklakeAmmSwapParams,
    pub account_metas: Vec<AccountMeta>,
}

/// Darklake AMM swap parameters
#[derive(Debug, Clone)]
pub struct DarklakeAmmSwapParams {
    pub amount_in: u64,
    pub is_x_to_y: bool,
    pub c_min: [u8; 32],
}

/// Keyed account for AMM operations
#[derive(Debug, Clone)]
pub struct KeyedAccount {
    pub key: Pubkey,
    pub account: AccountData,
}

/// AMM context for operations
#[derive(Debug, Clone)]
pub struct AmmContext {
    pub clock_ref: ClockRef,
}

/// Clock reference for AMM operations
#[derive(Debug, Clone)]
pub struct ClockRef {
    pub slot: u64,
    pub epoch: u64,
}

impl From<solana_sdk::clock::Clock> for ClockRef {
    fn from(clock: solana_sdk::clock::Clock) -> Self {
        Self {
            slot: clock.slot,
            epoch: clock.epoch,
        }
    }
}

/// Helper function to get account data from account map
pub fn try_get_account_data<'a>(account_map: &'a AccountMap, pubkey: &Pubkey) -> Result<&'a [u8]> {
    account_map
        .get(pubkey)
        .map(|account| account.data.as_slice())
        .ok_or_else(|| anyhow::anyhow!("Account not found: {}", pubkey))
}

/// Helper function to get account data and owner from account map
pub fn try_get_account_data_and_owner<'a>(
    account_map: &'a AccountMap,
    pubkey: &Pubkey,
) -> Result<(&'a [u8], &'a Pubkey)> {
    account_map
        .get(pubkey)
        .map(|account| (account.data.as_slice(), &account.owner))
        .ok_or_else(|| anyhow::anyhow!("Account not found: {}", pubkey))
}
