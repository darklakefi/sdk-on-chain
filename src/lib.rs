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
//! The SDK includes internal chain state tracking functions:
//! - **`load_pool`**: Loads pool data for internal state tracking
//! - **`update_accounts`**: Updates internal state with latest chain data
//! - **`get_order`**: Exception helper that bypasses internal cache and fetches the latest order state directly from the chain
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
//! // Initialize the SDK (no longer requires a keypair)
//! let mut sdk = DarklakeSDK::new("https://api.devnet.solana.com", CommitmentLevel::Confirmed);
//! ```
//!
//! ## ⚠️ Important: SOL/WSOL Handling
//!
//! **The Darklake DEX does not support direct SOL pairs - only WSOL (Wrapped SOL) pairs are supported.**
//!
//! ### Automatic Handling (Transaction Functions)
//! The transaction functions (`swap_tx`, `add_liquidity_tx`, `remove_liquidity_tx`) automatically handle SOL/WSOL conversion by:
//! - Adding wrap instructions when SOL is provided as input
//! - Adding unwrap instructions when WSOL is received as output
//!
//! ### Manual Handling (Instruction Functions)
//! The instruction functions (`swap_ix`, `finalize_ix`, `add_liquidity_ix`, `remove_liquidity_ix`) **do not** automatically handle SOL/WSOL conversion. When using these methods:
//! - You must manually add wrap/unwrap instructions if needed
//! - Ensure proper WSOL token account management
//! - Supply the `unwrap_wsol` parameter in `FinalizeParams` if necessary or add a WSOL token account closing.
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
//! use solana_sdk::pubkey::Pubkey;
//! use solana_sdk::signer::keypair::Keypair;
//!
//! // Define token mints
//! let token_in = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(); // WSOL (Wrapped SOL)
//! let token_out = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(); // USDC
//! let user_keypair = Keypair::new(); // Your wallet keypair
//! let user_pubkey = user_keypair.pubkey();
//!
//! // Get a quote first
//! let quote = sdk.quote(token_in, token_out, 1_000_000).await?; // 1 WSOL
//! println!("Expected output: {}", quote.out_amount);
//!
//! // Execute the swap - returns transaction and extra parameters needed for finalize
//! let (swap_tx, order_key, min_amount_out, salt) = sdk.swap_tx(
//!     token_in,
//!     token_out,
//!     1_000_000,  // 1 WSOL (in lamports)
//!     950_000,    // Minimum 0.95 USDC out (5% slippage)
//!     user_pubkey, // Token owner
//! ).await?;
//!
//! // Sign and send the swap transaction
//! let recent_blockhash = rpc_client.get_latest_blockhash().await?;
//! let swap_tx_signed = Transaction::new_signed_with_payer(
//!     &swap_tx.message.instructions,
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let swap_signature = rpc_client.send_and_confirm_transaction(&swap_tx_signed)?;
//!
//! // Generate the finalize transaction using the returned parameters
//! let finalize_tx = sdk.finalize_tx(
//!     order_key,
//!     true, // unwrap_wsol if output is WSOL
//!     min_amount_out,
//!     salt,
//!     None, // settle_signer (optional, defaults to order owner)
//! ).await?;
//!
//! // Sign and send the finalize transaction
//! let finalize_tx_signed = Transaction::new_signed_with_payer(
//!     &finalize_tx.message.instructions,
//!     Some(&user_pubkey),
//!     &[&user_keypair],
//!     recent_blockhash,
//! );
//! let finalize_signature = rpc_client.send_and_confirm_transaction(&finalize_tx_signed)?;
//!
//! println!("Swap signature: {}", swap_signature);
//! println!("Finalize signature: {}", finalize_signature);
//! ```
//!
//! ## 🔧 API Reference
//!
//! ### DarklakeSDK Methods
//!
//! #### Transaction Functions (`_tx`) - Fully Formatted Transactions
//!
//! - **`quote(token_in, token_out, amount_in)`** - Get a quote for a swap
//! - **`swap_tx(token_in, token_out, amount_in, min_amount_out, token_owner)`** - Generate swap transaction, returns `(Transaction, order_key, min_amount_out, salt)`
//! - **`finalize_tx(order_key, unwrap_wsol, min_out, salt, settle_signer)`** - Generate finalize transaction using parameters from swap_tx
//! - **`add_liquidity_tx(token_x, token_y, max_amount_x, max_amount_y, amount_lp, user)`** - Generate add liquidity transaction
//! - **`remove_liquidity_tx(token_x, token_y, min_amount_x, min_amount_y, amount_lp, user)`** - Generate remove liquidity transaction
//!
//! #### Instruction Functions (`_ix`) - Core Instructions
//!
//! - **`swap_ix(swap_params)`** - Generate swap instruction
//! - **`finalize_ix(finalize_params)`** - Generate finalize instruction
//! - **`add_liquidity_ix(add_liquidity_params)`** - Generate add liquidity instruction
//! - **`remove_liquidity_ix(remove_liquidity_params)`** - Generate remove liquidity instruction
//!
//! #### Internal State Management
//!
//! - **`load_pool(token_x, token_y)`** - Load pool data for internal state tracking
//! - **`update_accounts()`** - Update internal state with latest chain data
//! - **`get_order(user, commitment_level)`** - Get order data (bypasses internal cache, fetches latest state directly from chain)
//!
//! ## 🌐 Network Configuration
//!
//! SDK needs an rpc url which is used for on chain data fetching.
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
//!
//! **Note**: This SDK is for interacting with the Darklake DEX on Solana. Always test thoroughly on devnet before using on mainnet.

pub mod account_metas;
pub mod amm;
pub mod constants;
pub mod darklake_amm;
pub mod proof;
pub mod utils;
pub mod sdk;

pub use darklake_amm::DarklakeAmm;
pub use sdk::DarklakeSDK;
pub use account_metas::*;
pub use amm::*;