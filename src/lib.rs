//! # Darklake DEX SDK
//!
//! A standalone SDK for interacting with Darklake AMM pools on Solana. This SDK provides two main usage flows:
//!
//! 1. **Transaction Functions (`_tx`)**: Return fully formatted transaction that can be signed and sent
//! 2. **Instruction Functions (`_ix`)**: Return core instruction, allowing users to manage additional calls as needed
//!
//! > **📚 Detailed Examples**: For comprehensive examples and advanced usage patterns, see the [SDK Examples Repository](https://github.com/darklakefi/sdk-on-chain-examples).
//!
//! ## Internal State Management
//!
//! The functions are only necessary if `_ix` functions are used. As the SDK user is expected to call `load_poo` and `updated_accounts` before calling any `_ix`.
//!
//! The SDK includes internal chain state tracking functions:
//! - **`load_pool`**: Loads pool data for internal state tracking
//! - **`update_accounts`**: Updates internal state with latest chain data
//! - **`get_order`**: Exception helper that bypasses internal cache and fetches the latest order state directly from the chain. This is used to help reduce on-chain calls when only the order is needed. Also exports `Order` struct.
//!
//! ## 🚀 Quick Start
//!
//! ### Installation
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! darklake-sdk = "0.1.7"
//! ```
//!
//! ### Basic Setup
//!
//! ```rust
//! use darklake_sdk::DarklakeSDK;
//! use solana_sdk::commitment_config::CommitmentLevel;
//!
//! // Initialize the SDK
//! let mut sdk = DarklakeSDK::new(
//!     "https://api.devnet.solana.com",
//!     CommitmentLevel::Confirmed,
//!     true, // is_devnet
//!     None, // label (optional, up to 10 characters)
//!     None, // ref_code (optional, up to 20 characters)
//! )?;
//! ```
//!
//! #### Optional Parameters
//!
//! - **`label`**: Optional string up to 10 characters for identifying your application (Supply `None` if not needed)
//! - **`ref_code`**: Optional string up to 20 characters for referral tracking (Supply `None` if not needed)
//!
//! ## 📝 Important Notes
//!
//! ### Versioned Transactions
//! The SDK uses **Versioned Transactions** by default, which is the preferred approach for better performance and reduced transaction size. All transaction functions (`_tx`) return `VersionedTransaction` objects.
//!
//! ### Address Lookup Table
//! For devnet usage, you can import the pre-configured address lookup table (DEVNET_LOOKUP/MAINNET_LOOKUP):
//! ```rust
//! use darklake_sdk::DEVNET_LOOKUP;
//! ```
//!
//! ## ⚠️ Important: SOL/WSOL Handling
//!
//! **The Darklake DEX does not support direct SOL pairs - only WSOL (Wrapped SOL) pairs are supported.**
//!
//! ### Automatic Handling (Transaction Functions)
//! The transaction functions (`swap_tx`) automatically handle SOL/WSOL conversion by:
//! - Adding wrap instructions when SOL is provided as input
//! - Adding unwrap instructions when WSOL is received as output
//!
//! ### Manual Handling (Instruction Functions)
//! The instruction functions (`swap_ix`, `finalize_ix`) **do not** automatically handle SOL/WSOL wrapping. When using these methods:
//! - You must manually add wrap instructions if needed
//! - Supply the `unwrap_wsol` parameter in `FinalizeParamsIx` if necessary or add a WSOL token account closing.
//!
//! ## 📖 Usage Patterns
//!
//! ### 1. Transaction Functions (`_tx`) - Fully Formatted Transactions
//!
//! These functions return complete transactions ready to be signed and sent:
//!
//! #### Trading (Swap)
//!
//! ```rust
//! // Swap tx
//! let (swap_tx, order_key, min_out, salt) = sdk
//!     .swap_tx(token_mint_x, token_mint_y, 1_000, 1, user_keypair.pubkey())
//!     .await?;
//!
//! let tx = VersionedTransaction::try_new(swap_tx.message, &[&user_keypair])?;
//! let res = rpc_client.send_and_confirm_transaction_with_spinner(&tx)?;
//! ```
//!
//! #### Finalizing Swap
//!
//! ```rust
//! let finalize_tx: solana_sdk::transaction::VersionedTransaction = sdk
//!     .finalize_tx(order_key, unwrap_wsol, min_out, salt, None)
//!     .await?;
//!
//! let tx = VersionedTransaction::try_new(finalize_tx.message, &[&user_keypair])?;
//! ```
//!
//! ### 2. Instruction Functions (`_ix`) - Core Instructions
//!
//! These functions return core instructions, allowing you to manage additional calls as needed:
//!
//! #### Trading with Manual Control
//!
//! ```rust
//! sdk.load_pool(token_mint_x, token_mint_y).await?;
//!
//! sdk.update_accounts().await?;
//!
//! let salt = [1, 2, 3, 4, 5, 6, 7, 8];
//! let min_out = 1;
//!
//! let swap_params = SwapParamsIx {
//!     source_mint: token_mint_x,
//!     destination_mint: token_mint_y,
//!     token_transfer_authority: user_keypair.pubkey(),
//!     in_amount: 1_000,
//!     swap_mode: SwapMode::ExactIn,
//!     min_out,
//!     salt,
//! };
//!
//! let swap_ix = sdk.swap_ix(swap_params)?;
//!
//! let recent_blockhash = rpc_client
//!     .get_latest_blockhash()
//!     .context("Failed to get recent blockhash")?;
//!
//! let address_lookup_table = get_address_lookup_table(&rpc_client, DEVNET_LOOKUP).await?;
//!
//! let message_v0 = v0::Message::try_compile(
//!     &user_keypair.pubkey(),
//!     &[swap_ix],
//!     &[address_lookup_table.clone()],
//!     recent_blockhash,
//! )?;
//!
//! let mut transaction = VersionedTransaction {
//!     signatures: vec![],
//!     message: VersionedMessage::V0(message_v0),
//! };
//!
//! transaction.signatures = vec![user_keypair.sign_message(&transaction.message.serialize())];
//! ```
//!
//! #### Finalizing with Manual Control
//!
//! ```rust
//! let finalize_params = FinalizeParamsIx {
//!     settle_signer: user_keypair.pubkey(),
//!     order_owner: user_keypair.pubkey(),
//!     unwrap_wsol: false,      // Set to true if output is wrapped SOL
//!     min_out,                 // Same min_out as swap
//!     salt,                    // Same salt as swap
//!     output: order.d_out,     // Fetched from on chain order
//!     commitment: order.c_min, // Fetched from on chain order
//!     deadline: order.deadline, // Fetched from on chain order
//!     current_slot: rpc_client.get_slot()?,
//! };
//!
//! let compute_budget_ix: Instruction = ComputeBudgetInstruction::set_compute_unit_limit(500_000);
//!
//! let finalize_ix = sdk.finalize_ix(finalize_params)?;
//!
//! let recent_blockhash = rpc_client
//!     .get_latest_blockhash()
//!     .context("Failed to get recent blockhash")?;
//!
//! let message_v0 = v0::Message::try_compile(
//!     &user_keypair.pubkey(),
//!     &[compute_budget_ix, finalize_ix],
//!     &[address_lookup_table],
//!     recent_blockhash,
//! )?;
//!
//! let mut transaction = VersionedTransaction {
//!     signatures: vec![],
//!     message: VersionedMessage::V0(message_v0),
//! };
//!
//! transaction.signatures = vec![user_keypair.sign_message(&transaction.message.serialize())];
//! ```
//!
//! ## 🔧 API Reference
//!
//! ### DarklakeSDK Constructor
//!
//! #### `DarklakeSDK::new(rpc_endpoint, commitment_level, is_devnet, label, ref_code)`
//!
//! Creates a new Darklake SDK instance.
//!
//! **Parameters:**
//! - `rpc_endpoint: &str` - Solana RPC endpoint URL
//! - `commitment_level: CommitmentLevel` - Commitment level for RPC calls
//! - `is_devnet: bool` - Whether using devnet. Currently only devnet/mainnet supported.
//! - `label: Option<&str>` - Optional application/user label (max 10 characters). For example `Some("duck-ag")`.
//! - `ref_code: Option<&str>` - Optional referral code (max 20 characters)
//!
//! **Returns:** `Result<DarklakeSDK>`
//!
//! **Example:**
//! ```rust
//! let sdk = DarklakeSDK::new(
//!     "https://api.devnet.solana.com",
//!     CommitmentLevel::Confirmed,
//!     true, // is_devnet
//!     None, // label
//!     None, // ref_code
//! )?;
//! ```
//!
//! ### DarklakeSDK Methods
//!
//! #### Transaction Functions (`_tx`) - Fully Formatted Transactions
//!
//! - **`quote(token_in, token_out, amount_in)`** - Get a quote for a swap
//! - **`swap_tx(token_in, token_out, amount_in, min_amount_out, token_owner)`** - Generate swap transaction, returns `(VersionedTransaction, order_key, min_amount_out, salt)`
//! - **`finalize_tx(order_key, unwrap_wsol, min_out, salt, settle_signer)`** - Generate finalize transaction using parameters from swap_tx
//!
//! #### Instruction Functions (`_ix`) - Core Instructions
//!
//! - **`swap_ix(swap_params)`** - Generate swap instruction
//! - **`finalize_ix(finalize_params)`** - Generate finalize instruction
//!
//! #### Internal State Management
//!
//! - **`load_pool(token_x, token_y)`** - Load pool data for internal state tracking
//! - **`update_accounts()`** - Update internal state with latest chain data
//! - **`get_order(user, commitment_level)`** - Get order data (bypasses internal cache, fetches latest state directly from chain)
//!
//! ### Parameter Types
//!
//! #### SwapParamsIx
//! ```rust
//! pub struct SwapParamsIx {
//!     pub source_mint: Pubkey,
//!     pub destination_mint: Pubkey,
//!     pub token_transfer_authority: Pubkey,
//!     pub in_amount: u64,
//!     pub swap_mode: SwapMode,
//!     pub min_out: u64,
//!     pub salt: [u8; 8],
//! }
//! ```
//!
//! #### FinalizeParamsIx
//! ```rust
//! pub struct FinalizeParamsIx {
//!     pub settle_signer: Pubkey,
//!     pub order_owner: Pubkey,
//!     pub unwrap_wsol: bool, // Set to true if output is WSOL and you want to unwrap it to SOL
//!     pub min_out: u64,
//!     pub salt: [u8; 8],
//!     pub output: u64,
//!     pub commitment: [u8; 32],
//!     pub deadline: u64,
//!     pub current_slot: u64,
//! }
//! ```
//!
//! ## 🌐 Network Configuration
//!
//! SDK needs an rpc url which is used for on chain data fetching.
//!
//! ### Address Lookup Tables
//! The SDK provides pre-configured address lookup tables for use with versioned transaction:
//! - **`DEVNET_LOOKUP`**: Pre-configured address lookup table for devnet usage
//! - **`MAINNET_LOOKUP`**: Pre-configured address lookup table for mainnet usage
//!
//! ```rust
//! use darklake_sdk::DEVNET_LOOKUP;
//! ```
//!
//! ## 📈 Performance Considerations
//!
//! - **Proof generation**: currently non-async and blocking
//!
//! ## 📄 License
//!
//! MIT License - see LICENSE file for details.
//!
//! ## 🆘 Support
//!
//! For issues and questions:
//! - Check the examples in the repository
//! - Review the SDK source code
//! - Open an issue on the repository
//!
//! ---
//! **Note**: Make sure you are using the same commitment level both for your own rpc and the sdk, unless you know what you are doing.
//!
//! **Note**: This SDK is for interacting with the Darklake DEX on Solana. Always test thoroughly on devnet before using on mainnet.

mod account_metas;
mod amm;
mod constants;
mod darklake_amm;
mod proof;
mod reduced_amm_params;
mod sdk;
mod utils;

pub use sdk::DarklakeSDK;

pub use reduced_amm_params::{
    AddLiquidityParamsIx, FinalizeParamsIx, InitializePoolParamsIx, RemoveLiquidityParamsIx,
    SwapParamsIx,
};

pub use darklake_amm::Order;

pub use amm::SwapMode;

pub use constants::{DEVNET_LOOKUP, MAINNET_LOOKUP};
