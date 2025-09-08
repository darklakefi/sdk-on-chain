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

pub mod account_metas;
pub mod amm;
pub mod constants;
pub mod darklake_amm;
pub mod proof;
pub mod utils;

use std::{collections::HashMap, rc::Rc, str::FromStr};

// Re-export main types for easy access
pub use account_metas::*;
pub use amm::*;
use anchor_client::{solana_sdk::signer::keypair::Keypair, Client, Cluster};
pub use darklake_amm::{DarklakeAmm};
use spl_token::native_mint;

use crate::{
    constants::{AMM_CONFIG, DARKLAKE_PROGRAM_ID, POOL_SEED, SOL_MINT},
    darklake_amm::Order,
    utils::{generate_random_salt, get_wrap_sol_to_wsol_instructions, get_close_wsol_instructions},
};
use anyhow::{Context, Result};
use solana_rpc_client_api::config::RpcSendTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig, compute_budget::ComputeBudgetInstruction,
    instruction::Instruction, pubkey::Pubkey, signature::Signature, signer::Signer,
};
use tokio::time::{sleep, Duration};

/// Stateful Darklake SDK that holds RPC client and signer
pub struct DarklakeSDK {
    client: Client<Rc<Keypair>>,
    darklake_amm: Option<DarklakeAmm>,
    transaction_config: RpcSendTransactionConfig,
}

impl DarklakeSDK {
    /// Create a new Darklake SDK instance
    pub fn new(rpc_endpoint: &str, payer: Keypair) -> Self {
        let cluster = Cluster::from_str(rpc_endpoint).unwrap();
        let commitment_config = CommitmentConfig::finalized();
        let rc = Rc::new(payer);

        let transaction_config: RpcSendTransactionConfig = RpcSendTransactionConfig {
            skip_preflight: false,
            preflight_commitment: Some(commitment_config.commitment),
            encoding: None,
            max_retries: None,
            min_context_slot: None,
        };

        Self {
            client: Client::new_with_options(cluster, rc.clone(), commitment_config),
            darklake_amm: None,
            transaction_config,
        }
    }

    /// Get a quote for a swap
    ///
    /// # Arguments
    /// * `token_in` - The input token mint
    /// * `token_out` - The output token mint  
    /// * `amount_in` - The amount of input tokens
    ///
    /// # Returns
    /// Returns a `Quote` containing the expected output amount and other swap details
    pub async fn quote(
        &mut self,
        token_in: Pubkey,
        token_out: Pubkey,
        amount_in: u64,
    ) -> Result<Quote> {
        let rpc_client = self.client.program(DARKLAKE_PROGRAM_ID)?.rpc();

        let is_from_sol = token_in == SOL_MINT;
        let is_to_sol = token_out == SOL_MINT;

        let _token_in = if is_from_sol {
            native_mint::ID
        } else {
            token_in
        };
        let _token_out = if is_to_sol {
            native_mint::ID
        } else {
            token_out
        };

        let (pool_key, _token_x, _token_y) = Self::get_pool_address(token_in, token_out);

        if self.darklake_amm.is_none() || self.darklake_amm.as_ref().unwrap().key() != pool_key {
            self.load_pool(_token_x, _token_y).await?;
        }

        // update accounts
        let accounts_to_update = self.darklake_amm.as_ref().unwrap().get_accounts_to_update();
        let mut account_map = HashMap::new();
        for account_key in accounts_to_update {
            let account = rpc_client.get_account(&account_key).await?;
            account_map.insert(
                account_key,
                AccountData {
                    data: account.data,
                    owner: account.owner,
                },
            );
        }
        self.darklake_amm.as_mut().unwrap().update(&account_map)?;

        self.darklake_amm.as_ref().unwrap().quote(&QuoteParams {
            input_mint: token_in,
            amount: amount_in,
            swap_mode: SwapMode::ExactIn,
        })
    }

    /// Execute a swap
    ///
    /// # Arguments
    /// * `token_in` - The input token mint
    /// * `token_out` - The output token mint
    /// * `amount_in` - The amount of input tokens
    /// * `min_amount_out` - The minimum amount of output tokens expected
    /// * `token_owner` - Optional token owner keypair. If not provided, uses the payer as token owner
    ///
    /// # Returns
    /// Returns the transaction signature of the executed swap
    pub async fn swap(
        &mut self,
        token_in: Pubkey,
        token_out: Pubkey,
        amount_in: u64,
        min_amount_out: u64,
        token_owner: Option<Keypair>,
    ) -> Result<(Signature, Signature)> {
        let rpc_client = self.client.program(DARKLAKE_PROGRAM_ID)?.rpc();

        let is_from_sol = token_in == SOL_MINT;
        let is_to_sol = token_out == SOL_MINT;

        let _token_in = if is_from_sol {
            native_mint::ID
        } else {
            token_in
        };
        let _token_out = if is_to_sol {
            native_mint::ID
        } else {
            token_out
        };

        let (pool_key, _token_x, _token_y) = Self::get_pool_address(_token_in, _token_out);

        if self.darklake_amm.is_none() || self.darklake_amm.as_ref().unwrap().key() != pool_key {
            self.load_pool(_token_x, _token_y).await?;
        }

        // update accounts
        self.update_accounts().await?;

        let salt = generate_random_salt();

        let payer_pubkey: Pubkey = self.client.program(DARKLAKE_PROGRAM_ID).unwrap().payer();
        let token_owner_pubkey = match &token_owner {
            Some(token_owner) => token_owner.pubkey(),
            None => payer_pubkey,
        };

        let swap_params = SwapParams {
            source_mint: token_in,
            destination_mint: token_out,
            token_transfer_authority: token_owner_pubkey,
            in_amount: amount_in, // 1 token (assuming 6 decimals)
            swap_mode: SwapMode::ExactIn,
            min_out: min_amount_out, // 0.95 tokens out (5% slippage tolerance)
            salt,                    // Random salt for order uniqueness
        };

        let swap_and_account_metas = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .get_swap_and_account_metas(&swap_params)
            .context("Failed to get swap instruction and account metadata")?;

        let swap_instruction = Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: swap_and_account_metas.account_metas,
            data: swap_and_account_metas.data,
        };

        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(300_000);

        let program = self.client.program(DARKLAKE_PROGRAM_ID)?;

        let mut request_builder = program.request();
        request_builder = request_builder.instruction(compute_budget_ix);

        if is_from_sol {
            let sol_to_wsol_instructions = get_wrap_sol_to_wsol_instructions(token_owner_pubkey, amount_in)?;
            request_builder = request_builder.instruction(sol_to_wsol_instructions[0].clone());
            request_builder = request_builder.instruction(sol_to_wsol_instructions[1].clone());
            request_builder = request_builder.instruction(sol_to_wsol_instructions[2].clone());
        }

        request_builder = request_builder.instruction(swap_instruction);

        let swap_signature = if let Some(token_owner) = token_owner {
            request_builder
                .signer(token_owner)
                .send_with_spinner_and_config(self.transaction_config)
                .await?
        } else {
            request_builder
                .send_with_spinner_and_config(self.transaction_config)
                .await?
        };

        let order_key = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .get_order_pubkey(self.client.program(DARKLAKE_PROGRAM_ID).unwrap().payer())?;

        // Retry getting order data 5 times every 5 seconds
        let mut order_data = None;
        for attempt in 1..=5 {
            match rpc_client.get_account(&order_key).await {
                Ok(account) => {
                    order_data = Some(account);
                    break;
                }
                Err(e) => {
                    if attempt == 5 {
                        return Err(e).context("Failed to get order data after 5 attempts");
                    }
                    log::warn!(
                        "Attempt {} failed to get order data: {}. Retrying in 5 seconds...",
                        attempt,
                        e
                    );
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        // Verify we got the order data
        if order_data.is_none() {
            return Err(anyhow::anyhow!(
                "Failed to get order data after all retry attempts"
            ));
        }

        let order = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .parse_order_data(&order_data.unwrap().data)?;

        // update accounts
        let accounts_to_update = self.darklake_amm.as_ref().unwrap().get_accounts_to_update();
        let mut account_map = HashMap::new();
        for account_key in accounts_to_update {
            let account = rpc_client.get_account(&account_key).await?;
            account_map.insert(
                account_key,
                AccountData {
                    data: account.data,
                    owner: account.owner,
                },
            );
        }
        self.darklake_amm.as_mut().unwrap().update(&account_map)?;

        let finalize_params = FinalizeParams {
            settle_signer: payer_pubkey,  // Always payer
            order_owner: token_owner_pubkey, // Use token_owner (or payer as fallback)
            unwrap_wsol: is_to_sol,           // Set to true if output is wrapped SOL
            min_out: swap_params.min_out, // Same min_out as swap
            salt: swap_params.salt,       // Same salt as swap
            output: order.d_out,          // Will be populated by the SDK
            commitment: swap_and_account_metas.swap.c_min, // Will be populated by the SDK
            deadline: order.deadline,
            current_slot: rpc_client
                .get_slot_with_commitment(CommitmentConfig::processed())
                .await?,
        };

        let finalize_and_account_metas = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .get_finalize_and_account_metas(&finalize_params)?;

        let finalize_instruction = Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: finalize_and_account_metas.account_metas(),
            data: finalize_and_account_metas.data(),
        };

        let settle_transaction_config: RpcSendTransactionConfig = RpcSendTransactionConfig {
            skip_preflight: false,
            preflight_commitment: Some(CommitmentConfig::processed().commitment),
            encoding: None,
            max_retries: None,
            min_context_slot: None,
        };

        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(500_000);

        let program = self.client.program(DARKLAKE_PROGRAM_ID)?;
        let request_builder = program.request();

        let finalize_signature = request_builder
            .instruction(compute_budget_ix)
            .instruction(finalize_instruction)
            .send_with_spinner_and_config(settle_transaction_config)
            .await?;

        Ok((swap_signature, finalize_signature))
    }

    pub async fn add_liquidity(
        &mut self,
        token_x: Pubkey,
        token_y: Pubkey,
        max_amount_x: u64,
        max_amount_y: u64,
        amount_lp: u64,
    ) -> Result<Signature> {
        let rpc_client = self.client.program(DARKLAKE_PROGRAM_ID)?.rpc();

        let is_x_sol = token_x == SOL_MINT;
        let is_y_sol = token_y == SOL_MINT;

        let token_x_post_sol = if is_x_sol {
            native_mint::ID
        } else {
            token_x
        };
        let token_y_post_sol = if is_y_sol {
            native_mint::ID
        } else {
            token_y
        };


        let (pool_key, _token_x, _token_y) = Self::get_pool_address(token_x_post_sol, token_y_post_sol);

        let max_amount_x = if _token_x != token_x_post_sol {
            max_amount_y
        } else {
            max_amount_x
        };
        let max_amount_y = if _token_x != token_x_post_sol {
            max_amount_x
        } else {
            max_amount_y
        };

        if self.darklake_amm.is_none() || self.darklake_amm.as_ref().unwrap().key() != pool_key {
            self.load_pool(_token_x, _token_y).await?;
        }

        // update accounts
        self.update_accounts().await?;

        let payer_pubkey: Pubkey = self.client.program(DARKLAKE_PROGRAM_ID).unwrap().payer();

        let add_liquidity_params = AddLiquidityParams {
            amount_lp,
            max_amount_x,
            max_amount_y,
            user: payer_pubkey,
        };

        let add_liquidity_and_account_metas = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .get_add_liquidity_and_account_metas(&add_liquidity_params)?;

        let add_liquidity_instruction = Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: add_liquidity_and_account_metas.account_metas,
            data: add_liquidity_and_account_metas.data,
        };

        let program = self.client.program(DARKLAKE_PROGRAM_ID)?;
        let mut request_builder = program.request();

        if is_x_sol {
            let sol_to_wsol_instructions = get_wrap_sol_to_wsol_instructions(payer_pubkey, max_amount_x)?;
            request_builder = request_builder.instruction(sol_to_wsol_instructions[0].clone());
            request_builder = request_builder.instruction(sol_to_wsol_instructions[1].clone());
            request_builder = request_builder.instruction(sol_to_wsol_instructions[2].clone());
        } else if is_y_sol {
            let sol_to_wsol_instructions = get_wrap_sol_to_wsol_instructions(payer_pubkey, max_amount_y)?;
            request_builder = request_builder.instruction(sol_to_wsol_instructions[0].clone());
            request_builder = request_builder.instruction(sol_to_wsol_instructions[1].clone());
            request_builder = request_builder.instruction(sol_to_wsol_instructions[2].clone());
        }

        let add_liquidity_signature = request_builder
            .instruction(add_liquidity_instruction)
            .send_with_spinner_and_config(self.transaction_config)
            .await?;

        Ok(add_liquidity_signature)
    }

    pub async fn remove_liquidity(
        &mut self,
        token_x: Pubkey,
        token_y: Pubkey,
        min_amount_x: u64,
        min_amount_y: u64,
        amount_lp: u64,
    ) -> Result<Signature> {
        let rpc_client = self.client.program(DARKLAKE_PROGRAM_ID)?.rpc();

        let is_x_sol = token_x == SOL_MINT;
        let is_y_sol = token_y == SOL_MINT;

        let token_x_post_sol = if is_x_sol {
            native_mint::ID
        } else {
            token_x
        };
        let token_y_post_sol = if is_y_sol {
            native_mint::ID
        } else {
            token_y
        };

        let (pool_key, _token_x, _token_y) = Self::get_pool_address(token_x_post_sol, token_y_post_sol);

        let min_amount_x = if _token_x != token_x_post_sol {
            min_amount_y
        } else {
            min_amount_x
        };
        let min_amount_y = if _token_x != token_x_post_sol {
            min_amount_x
        } else {
            min_amount_y
        };

        if self.darklake_amm.is_none() || self.darklake_amm.as_ref().unwrap().key() != pool_key {
            self.load_pool(_token_x, _token_y).await?;
        }

        // update accounts
        self.update_accounts().await?;

        let payer_pubkey: Pubkey = self.client.program(DARKLAKE_PROGRAM_ID).unwrap().payer();

        let remove_liquidity_params = RemoveLiquidityParams {
            amount_lp,
            min_amount_x,
            min_amount_y,
            user: payer_pubkey,
        };

        let remove_liquidity_and_account_metas = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .get_remove_liquidity_and_account_metas(&remove_liquidity_params)?;

        let remove_liquidity_instruction = Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: remove_liquidity_and_account_metas.account_metas,
            data: remove_liquidity_and_account_metas.data,
        };

        let program = self.client.program(DARKLAKE_PROGRAM_ID)?;
        let mut request_builder = program.request();

        // Add close WSOL instructions if either token is SOL (user can't have multiple WSOL accounts)
        if is_x_sol || is_y_sol {
            let close_wsol_instructions = get_close_wsol_instructions(payer_pubkey)?;
            request_builder = request_builder.instruction(close_wsol_instructions[0].clone());
            request_builder = request_builder.instruction(close_wsol_instructions[1].clone());
        }

        let remove_liquidity_signature = request_builder
            .instruction(remove_liquidity_instruction)
            .send_with_spinner_and_config(self.transaction_config)
            .await?;

        Ok(remove_liquidity_signature)
    }

    // MANUAL HANDLING (these are prone to changes in the future)

    // before calling swap_ix/finalize_ix/add_liquidity_ix/remove_liquidity_ix -
    // load_pool has to be called at least once before and update_accounts before each function call

    /// Create a new Darklake AMM instance from account data
    pub async fn load_pool(&mut self, token_x: Pubkey, token_y: Pubkey) -> Result<(Pubkey, Pubkey, Pubkey)> {
        let (pool_key, _, _) = Self::get_pool_address(token_x, token_y);

        let rpc_client = self.client.program(DARKLAKE_PROGRAM_ID)?.rpc();
        let pool_account_data = rpc_client.get_account(&pool_key).await?;

        let pool_key_and_account = KeyedAccount {
            key: pool_key,
            account: AccountData {
                data: pool_account_data.data.to_vec(),
                owner: DARKLAKE_PROGRAM_ID,
            },
        };

        self.darklake_amm = Some(DarklakeAmm::load_pool(&pool_key_and_account)?);

        // returns sorted token mints
        Ok((pool_key, token_x, token_y))
    }

    pub async fn update_accounts(&mut self) -> Result<()> {
        let rpc_client = self.client.program(DARKLAKE_PROGRAM_ID)?.rpc();

        let accounts_to_update = self.darklake_amm.as_ref().unwrap().get_accounts_to_update();
        let mut account_map = HashMap::new();
        for account_key in accounts_to_update {
            let account = rpc_client.get_account(&account_key).await?;
            account_map.insert(
                account_key,
                AccountData {
                    data: account.data,
                    owner: account.owner,
                },
            );
        }
        self.darklake_amm.as_mut().unwrap().update(&account_map)?;

        Ok(())
    }

    pub async fn swap_ix(&mut self, swap_params: SwapParams) -> Result<Instruction> {
        let swap_and_account_metas = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .get_swap_and_account_metas(&swap_params)
            .context("Failed to get swap instruction and account metadata")?;

        Ok(Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: swap_and_account_metas.account_metas,
            data: swap_and_account_metas.data,
        })
    }

    // does not require load_pool or update_accounts is a standalone function after new() is called
    pub async fn get_order(&mut self, user: Pubkey) -> Result<Order> {
        let rpc_client = self.client.program(DARKLAKE_PROGRAM_ID)?.rpc();

        let order_key = self.darklake_amm.as_ref().unwrap().get_order_pubkey(user)?;

        let order_data = rpc_client.get_account(&order_key).await?;

        let order = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .parse_order_data(&order_data.data)?;

        Ok(order)
    }

    pub async fn finalize_ix(&mut self, finalize_params: FinalizeParams) -> Result<Instruction> {
        let finalize_and_account_metas = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .get_finalize_and_account_metas(&finalize_params)?;

        Ok(Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: finalize_and_account_metas.account_metas(),
            data: finalize_and_account_metas.data(),
        })
    }

    pub async fn add_liquidity_ix(
        &mut self,
        add_liquidity_params: AddLiquidityParams,
    ) -> Result<Instruction> {
        let add_liquidity_and_account_metas = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .get_add_liquidity_and_account_metas(&add_liquidity_params)?;

        Ok(Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: add_liquidity_and_account_metas.account_metas,
            data: add_liquidity_and_account_metas.data,
        })
    }

    pub async fn remove_liquidity_ix(
        &mut self,
        remove_liquidity_params: RemoveLiquidityParams,
    ) -> Result<Instruction> {
        let remove_liquidity_and_account_metas = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .get_remove_liquidity_and_account_metas(&remove_liquidity_params)?;

        Ok(Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: remove_liquidity_and_account_metas.account_metas,
            data: remove_liquidity_and_account_metas.data,
        })
    }

    /// Getters
    /// Get the signer's public key
    pub fn signer_pubkey(&self) -> Pubkey {
        self.client.program(DARKLAKE_PROGRAM_ID).unwrap().payer()
    }

    /// Helpers internal methods
    /// Get the pool address for a token pair
    fn get_pool_address(token_mint_x: Pubkey, token_mint_y: Pubkey) -> (Pubkey, Pubkey, Pubkey) {
        // Convert token mints to bytes and ensure x is always below y by lexicographical order
        let (ordered_x, ordered_y) = if token_mint_x < token_mint_y {
            (token_mint_x, token_mint_y)
        } else {
            (token_mint_y, token_mint_x)
        };

        let pool_key = Pubkey::find_program_address(
            &[
                POOL_SEED,
                AMM_CONFIG.as_ref(),
                ordered_x.as_ref(),
                ordered_y.as_ref(),
            ],
            &DARKLAKE_PROGRAM_ID,
        )
        .0;

        (pool_key, ordered_x, ordered_y)
    }
}
