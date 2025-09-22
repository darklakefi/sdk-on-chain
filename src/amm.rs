use anchor_lang::prelude::AccountMeta;
use anyhow::Result;
use solana_sdk::pubkey::Pubkey;

use crate::{darklake_amm::Order, proof::proof_generator::GeneratedProof};

/// Core AMM trait for Darklake DEX operations
pub(crate) trait Amm: Send + Sync {
    /// Deserialize the AMM from a keyed account
    fn load_pool(pool: &KeyedAccount) -> Result<Self>
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

    // Darklake specific

    /// Get settle parameters and account metadata
    fn get_settle_and_account_metas(
        &self,
        settle_params: &SettleParams,
        proof_params: &ProofParams,
    ) -> Result<SettleAndAccountMetas>;

    /// Get order pubkey
    fn get_order_pubkey(&self, user: &Pubkey) -> Result<Pubkey>;

    /// Get order output
    fn parse_order_data(&self, order_data: &[u8]) -> Result<Order>;

    /// Check if order is expired
    fn is_order_expired(&self, order_data: &[u8], current_slot: u64) -> Result<bool>;

    /// Get cancel parameters and account metadata
    fn get_cancel_and_account_metas(
        &self,
        cancel_params: &CancelParams,
        proof_params: &ProofParams,
    ) -> Result<CancelAndAccountMetas>;

    /// Get slash parameters and account metadata
    fn get_slash_and_account_metas(
        &self,
        slash_params: &SlashParams,
    ) -> Result<SlashAndAccountMetas>;

    /// Get slash parameters and account metadata
    fn get_initialize_pool_and_account_metas(
        &self,
        initialize_pool_params: &InitializePoolParams,
        is_devnet: bool,
    ) -> Result<InitializePoolAndAccountMetas>;

    /// Get add liquidity parameters and account metadata
    fn get_add_liquidity_and_account_metas(
        &self,
        add_liquidity_params: &AddLiquidityParams,
    ) -> Result<AddLiquidityAndAccountMetas>;

    /// Get remove liquidity parameters and account metadata
    fn get_remove_liquidity_and_account_metas(
        &self,
        remove_liquidity_params: &RemoveLiquidityParams,
    ) -> Result<RemoveLiquidityAndAccountMetas>;
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
    pub token_transfer_authority: Pubkey,
    pub in_amount: u64,
    pub swap_mode: SwapMode,
    pub min_out: u64,
    pub salt: [u8; 8],
    pub label: Option<[u8; 21]>,
}

/// Settle parameters
#[derive(Debug, Clone)]
pub struct SettleParams {
    pub settle_signer: Pubkey,
    pub order_owner: Pubkey,
    pub unwrap_wsol: bool,
    pub min_out: u64,
    pub salt: [u8; 8],
    pub output: u64,
    pub commitment: [u8; 32],
    pub deadline: u64,
    pub current_slot: u64,
    pub ref_code: Option<[u8; 20]>,
    pub label: Option<[u8; 21]>,
}

/// Cancel parameters
#[derive(Debug, Clone)]
pub struct CancelParams {
    pub settle_signer: Pubkey,
    pub order_owner: Pubkey,
    pub min_out: u64,
    pub salt: [u8; 8],
    pub output: u64,
    pub commitment: [u8; 32],
    pub deadline: u64,
    pub current_slot: u64,
    pub label: Option<[u8; 21]>,
}

/// Slash parameters
#[derive(Debug, Clone)]
pub struct SlashParams {
    pub settle_signer: Pubkey,
    pub order_owner: Pubkey,
    pub deadline: u64,
    pub current_slot: u64,
    pub label: Option<[u8; 21]>,
}

/// Add liquidity parameters
#[derive(Debug, Clone)]
pub struct AddLiquidityParams {
    pub user: Pubkey,
    pub amount_lp: u64, // lp to mint
    pub max_amount_x: u64,
    pub max_amount_y: u64,
    pub ref_code: Option<[u8; 20]>,
    pub label: Option<[u8; 21]>,
}

/// Initialize pool parameters
#[derive(Debug, Clone)]
pub struct InitializePoolParams {
    pub user: Pubkey,
    pub token_x: Pubkey,
    pub token_x_program: Pubkey,
    pub token_y: Pubkey,
    pub token_y_program: Pubkey,
    pub amount_x: u64,
    pub amount_y: u64,
    pub label: Option<[u8; 21]>,
}

/// Remove liquidity parameters
#[derive(Debug, Clone)]
pub struct RemoveLiquidityParams {
    pub user: Pubkey,
    pub amount_lp: u64, // lp to burn
    pub min_amount_x: u64,
    pub min_amount_y: u64,
    pub label: Option<[u8; 21]>,
}

/// Finalize parameters
#[derive(Debug, Clone)]
pub struct FinalizeParams {
    pub settle_signer: Pubkey,
    pub order_owner: Pubkey,
    pub unwrap_wsol: bool,
    pub min_out: u64,
    pub salt: [u8; 8],
    pub output: u64,
    pub commitment: [u8; 32],
    pub deadline: u64,
    pub current_slot: u64,
    pub ref_code: Option<[u8; 20]>,
    pub label: Option<[u8; 21]>,
}

/// Swap result with account metadata
#[derive(Debug, Clone)]
pub struct SwapAndAccountMetas {
    pub discriminator: [u8; 8],
    pub swap: DarklakeAmmSwapParams,
    pub data: Vec<u8>,
    pub account_metas: Vec<AccountMeta>,
}

/// Settle result with account metadata
#[derive(Debug, Clone)]
pub struct SettleAndAccountMetas {
    pub discriminator: [u8; 8],
    pub settle: DarklakeAmmSettleParams,
    pub data: Vec<u8>,
    pub account_metas: Vec<AccountMeta>,
}

/// Cancel result with account metadata
#[derive(Debug, Clone)]
pub struct CancelAndAccountMetas {
    pub discriminator: [u8; 8],
    pub cancel: DarklakeAmmCancelParams,
    pub data: Vec<u8>,
    pub account_metas: Vec<AccountMeta>,
}

/// Add liquidity result with account metadata
#[derive(Debug, Clone)]
pub struct AddLiquidityAndAccountMetas {
    pub discriminator: [u8; 8],
    pub add_liquidity: DarklakeAmmAddLiquidityParams,
    pub data: Vec<u8>,
    pub account_metas: Vec<AccountMeta>,
}

/// Remove liquidity result with account metadata
#[derive(Debug, Clone)]
pub struct RemoveLiquidityAndAccountMetas {
    pub discriminator: [u8; 8],
    pub remove_liquidity: DarklakeAmmRemoveLiquidityParams,
    pub data: Vec<u8>,
    pub account_metas: Vec<AccountMeta>,
}

/// Finalize result with account metadata

#[derive(Debug, Clone)]
pub enum FinalizeAndAccountMetas {
    Settle(SettleAndAccountMetas),
    Cancel(CancelAndAccountMetas),
    Slash(SlashAndAccountMetas),
}

impl FinalizeAndAccountMetas {
    pub fn data(&self) -> Vec<u8> {
        match self {
            FinalizeAndAccountMetas::Settle(settle) => settle.data.clone(),
            FinalizeAndAccountMetas::Cancel(cancel) => cancel.data.clone(),
            FinalizeAndAccountMetas::Slash(slash) => slash.data.clone(),
        }
    }

    pub fn account_metas(&self) -> Vec<AccountMeta> {
        match self {
            FinalizeAndAccountMetas::Settle(settle) => settle.account_metas.clone(),
            FinalizeAndAccountMetas::Cancel(cancel) => cancel.account_metas.clone(),
            FinalizeAndAccountMetas::Slash(slash) => slash.account_metas.clone(),
        }
    }
}

/// Slash result with account metadata

#[derive(Debug, Clone)]
pub struct SlashAndAccountMetas {
    pub discriminator: [u8; 8],
    pub slash: DarklakeAmmSlashParams,
    pub data: Vec<u8>,
    pub account_metas: Vec<AccountMeta>,
}

/// Initialize pool result with account metadata
#[derive(Debug, Clone)]
pub struct InitializePoolAndAccountMetas {
    pub discriminator: [u8; 8],
    pub initialize_pool: DarklakeAmmInitializePoolParams,
    pub data: Vec<u8>,
    pub account_metas: Vec<AccountMeta>,
}

/// Darklake AMM swap parameters
#[derive(Debug, Clone)]
pub struct DarklakeAmmSwapParams {
    pub amount_in: u64,
    pub is_swap_x_to_y: bool,
    pub c_min: [u8; 32],
    pub label: Option<[u8; 21]>,
}

/// Darklake AMM settle parameters
#[derive(Debug, Clone)]
pub struct DarklakeAmmSettleParams {
    pub proof_a: [u8; 64],
    pub proof_b: [u8; 128],
    pub proof_c: [u8; 64],
    pub public_signals: [[u8; 32]; 2],
    pub unwrap_wsol: bool,
    pub ref_code: Option<[u8; 20]>,
    pub label: Option<[u8; 21]>,
}

/// Darklake AMM settle parameters
#[derive(Debug, Clone)]
pub struct DarklakeAmmCancelParams {
    pub proof_a: [u8; 64],
    pub proof_b: [u8; 128],
    pub proof_c: [u8; 64],
    pub public_signals: [[u8; 32]; 2],
    pub label: Option<[u8; 21]>,
}

/// Darklake AMM slash parameters
#[derive(Debug, Clone)]
pub struct DarklakeAmmSlashParams {
    pub label: Option<[u8; 21]>,
}

/// Darklake AMM initialize pool parameters
#[derive(Debug, Clone)]
pub struct DarklakeAmmInitializePoolParams {
    pub amount_x: u64,
    pub amount_y: u64,
    pub label: Option<[u8; 21]>,
}

/// Darklake AMM add liquidity parameters
#[derive(Debug, Clone)]
pub struct DarklakeAmmAddLiquidityParams {
    pub amount_lp: u64,
    pub max_amount_x: u64,
    pub max_amount_y: u64,
    pub ref_code: Option<[u8; 20]>,
    pub label: Option<[u8; 21]>,
}

/// Darklake AMM remove liquidity parameters
#[derive(Debug, Clone)]
pub struct DarklakeAmmRemoveLiquidityParams {
    pub amount_lp: u64,
    pub min_amount_x: u64,
    pub min_amount_y: u64,
    pub label: Option<[u8; 21]>,
}

/// Keyed account for AMM operations
#[derive(Debug, Clone)]
pub struct KeyedAccount {
    pub key: Pubkey,
    pub account: AccountData,
}

#[derive(Debug, Clone)]
pub(crate) struct ProofCircuitPaths {
    pub wasm_path: String,
    pub zkey_path: String,
    pub r1cs_path: String,
}

#[derive(Debug, Clone)]
pub struct ProofParams {
    pub generated_proof: GeneratedProof,
    pub public_inputs: [[u8; 32]; 2],
}
/// Helper function to get account data from account map
pub(crate) fn try_get_account_data<'a>(
    account_map: &'a AccountMap,
    pubkey: &Pubkey,
) -> Result<&'a [u8]> {
    account_map
        .get(pubkey)
        .map(|account| account.data.as_slice())
        .ok_or_else(|| anyhow::anyhow!("Account not found: {}", pubkey))
}

/// Helper function to get account data and owner from account map
pub(crate) fn try_get_account_data_and_owner<'a>(
    account_map: &'a AccountMap,
    pubkey: &Pubkey,
) -> Result<(&'a [u8], &'a Pubkey)> {
    account_map
        .get(pubkey)
        .map(|account| (account.data.as_slice(), &account.owner))
        .ok_or_else(|| anyhow::anyhow!("Account not found: {}", pubkey))
}
