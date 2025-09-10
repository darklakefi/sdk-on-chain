use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use spl_token::native_mint;

use crate::{
    amm::{
        AccountData, AddLiquidityParams, Amm, FinalizeParams, KeyedAccount, ProofCircuitPaths,
        ProofParams, Quote, QuoteParams, RemoveLiquidityParams, SwapMode, SwapParams,
    },
    constants::{AMM_CONFIG, DARKLAKE_PROGRAM_ID, POOL_SEED, SOL_MINT},
    darklake_amm::{DarklakeAmm, Order},
    proof::proof_generator::find_circuit_path,
    utils::{generate_random_salt, get_close_wsol_instructions, get_wrap_sol_to_wsol_instructions},
};
use anyhow::{Context, Result};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    transaction::Transaction,
};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

/// Stateful Darklake SDK that holds RPC client and signer
pub struct DarklakeSDK {
    rpc_client: RpcClient,
    darklake_amm: Option<DarklakeAmm>,
    settle_paths: ProofCircuitPaths,
    cancel_paths: ProofCircuitPaths,
}

impl DarklakeSDK {
    /// Create a new Darklake SDK instance
    pub fn new(rpc_endpoint: &str, commitment_level: CommitmentLevel) -> Self {
        let commitment_config = CommitmentConfig {
            commitment: commitment_level,
        };

        let settle_file_prefix = "settle";
        let cancel_file_prefix = "cancel";

        let settle_wasm_path = find_circuit_path(&format!("{}.wasm", settle_file_prefix));
        let settle_zkey_path = find_circuit_path(&format!("{}_final.zkey", settle_file_prefix));
        let settle_r1cs_path = find_circuit_path(&format!("{}.r1cs", settle_file_prefix));

        let cancel_wasm_path = find_circuit_path(&format!("{}.wasm", cancel_file_prefix));
        let cancel_zkey_path = find_circuit_path(&format!("{}_final.zkey", cancel_file_prefix));
        let cancel_r1cs_path = find_circuit_path(&format!("{}.r1cs", cancel_file_prefix));

        Self {
            rpc_client: RpcClient::new_with_commitment(rpc_endpoint.to_string(), commitment_config),
            darklake_amm: None,
            settle_paths: ProofCircuitPaths {
                wasm_path: settle_wasm_path,
                zkey_path: settle_zkey_path,
                r1cs_path: settle_r1cs_path,
            },
            cancel_paths: ProofCircuitPaths {
                wasm_path: cancel_wasm_path,
                zkey_path: cancel_zkey_path,
                r1cs_path: cancel_r1cs_path,
            },
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
    /// * `token_owner` - The token owner public key
    ///
    /// # Returns
    /// Returns the tx signature of the executed swap
    pub async fn swap_tx(
        &mut self,
        token_in: Pubkey,
        token_out: Pubkey,
        amount_in: u64,
        min_amount_out: u64,
        token_owner: Pubkey,
    ) -> Result<(Transaction, Pubkey, u64, [u8; 8])> {
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

        let swap_params = SwapParams {
            source_mint: token_in,
            destination_mint: token_out,
            token_transfer_authority: token_owner,
            in_amount: amount_in, // 1 token (assuming 6 decimals)
            swap_mode: SwapMode::ExactIn,
            min_out: min_amount_out, // 0.95 tokens out (5% slippage tolerance)
            salt,                    // Random salt for order uniqueness
        };

        let swap_instruction = self.swap_ix(swap_params).await?;

        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(300_000);

        let mut instructions = vec![compute_budget_ix];

        if is_from_sol {
            let sol_to_wsol_instructions =
                get_wrap_sol_to_wsol_instructions(token_owner, amount_in)?;
            instructions.push(sol_to_wsol_instructions[0].clone());
            instructions.push(sol_to_wsol_instructions[1].clone());
            instructions.push(sol_to_wsol_instructions[2].clone());
        }

        instructions.push(swap_instruction);

        // let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;

        let message = Message::new(
            &instructions,
            Some(&token_owner),
            // recent_blockhash,
        );

        let swap_transaction = Transaction::new_unsigned(message);

        let order_key = self
            .darklake_amm
            .as_ref()
            .unwrap()
            .get_order_pubkey(token_owner)?;

        Ok((swap_transaction, order_key, min_amount_out, salt))
    }

    pub async fn finalize_tx(
        &mut self,
        order_key: Pubkey,
        unwrap_wsol: bool,
        min_out: u64,
        salt: [u8; 8],
        settle_signer: Option<Pubkey>,
    ) -> Result<Transaction> {
        // Retry getting order data 5 times every 5 seconds
        let mut order_data = None;
        for attempt in 1..=5 {
            match self.rpc_client.get_account(&order_key).await {
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
        self.update_accounts().await?;

        let settler = settle_signer.unwrap_or(order.trader);
        let create_wsol_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &settler,
                &settler,
                &native_mint::ID,
                &spl_token::ID,
            );

        let finalize_params = FinalizeParams {
            settle_signer: settler,    // who settles the order
            order_owner: order.trader, // who owns the order
            unwrap_wsol,               // Set to true if output is wrapped SOL
            min_out,                   // Same min_out as swap
            salt,                      // Same salt as swap
            output: order.d_out,       // Will be populated by the SDK
            commitment: order.c_min,   // Will be populated by the SDK
            deadline: order.deadline,
            current_slot: self
                .rpc_client
                .get_slot_with_commitment(CommitmentConfig::processed())
                .await?,
        };

        let finalize_instruction = self.finalize_ix(finalize_params).await?;

        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(500_000);

        let instructions = vec![compute_budget_ix, create_wsol_ata_ix, finalize_instruction];

        let finalize_transaction =
            Transaction::new_unsigned(Message::new(&instructions, Some(&settler)));

        Ok(finalize_transaction)
    }

    pub async fn add_liquidity_tx(
        &mut self,
        token_x: Pubkey,
        token_y: Pubkey,
        max_amount_x: u64,
        max_amount_y: u64,
        amount_lp: u64,
        user: Pubkey,
    ) -> Result<Transaction> {
        let is_x_sol = token_x == SOL_MINT;
        let is_y_sol = token_y == SOL_MINT;

        let token_x_post_sol = if is_x_sol { native_mint::ID } else { token_x };
        let token_y_post_sol = if is_y_sol { native_mint::ID } else { token_y };

        let (pool_key, _token_x, _token_y) =
            Self::get_pool_address(token_x_post_sol, token_y_post_sol);

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

        let add_liquidity_params = AddLiquidityParams {
            amount_lp,
            max_amount_x,
            max_amount_y,
            user,
        };

        let add_liquidity_instruction = self.add_liquidity_ix(add_liquidity_params).await?;

        let mut instructions = vec![];
        if is_x_sol {
            let sol_to_wsol_instructions = get_wrap_sol_to_wsol_instructions(user, max_amount_x)?;
            instructions.push(sol_to_wsol_instructions[0].clone());
            instructions.push(sol_to_wsol_instructions[1].clone());
            instructions.push(sol_to_wsol_instructions[2].clone());
        } else if is_y_sol {
            let sol_to_wsol_instructions = get_wrap_sol_to_wsol_instructions(user, max_amount_y)?;
            instructions.push(sol_to_wsol_instructions[0].clone());
            instructions.push(sol_to_wsol_instructions[1].clone());
            instructions.push(sol_to_wsol_instructions[2].clone());
        }

        instructions.push(add_liquidity_instruction);

        let add_liquidity_transaction =
            Transaction::new_unsigned(Message::new(&instructions, Some(&user)));

        Ok(add_liquidity_transaction)
    }

    pub async fn remove_liquidity_tx(
        &mut self,
        token_x: Pubkey,
        token_y: Pubkey,
        min_amount_x: u64,
        min_amount_y: u64,
        amount_lp: u64,
        user: Pubkey,
    ) -> Result<Transaction> {
        let is_x_sol = token_x == SOL_MINT;
        let is_y_sol = token_y == SOL_MINT;

        let token_x_post_sol = if is_x_sol { native_mint::ID } else { token_x };
        let token_y_post_sol = if is_y_sol { native_mint::ID } else { token_y };

        let (pool_key, _token_x, _token_y) =
            Self::get_pool_address(token_x_post_sol, token_y_post_sol);

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

        let (token_x_owner, token_y_owner) = self.darklake_amm.as_ref().unwrap().get_token_owners();

        // make sure the user has the token accounts
        let create_token_x_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &user,
                &user,
                &_token_x,
                &token_x_owner,
            );

        let create_token_y_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &user,
                &user,
                &_token_y,
                &token_y_owner,
            );

        let remove_liquidity_params = RemoveLiquidityParams {
            amount_lp,
            min_amount_x,
            min_amount_y,
            user,
        };

        let remove_liquidity_instruction =
            self.remove_liquidity_ix(remove_liquidity_params).await?;

        let mut instructions = vec![
            create_token_x_ata_ix,
            create_token_y_ata_ix,
            remove_liquidity_instruction,
        ];

        // Add close WSOL instructions if either token is SOL (user can't have multiple WSOL accounts)
        if is_x_sol || is_y_sol {
            let close_wsol_instructions = get_close_wsol_instructions(user)?;
            instructions.push(close_wsol_instructions[0].clone());
            instructions.push(close_wsol_instructions[1].clone());
        }

        let remove_liquidity_transaction =
            Transaction::new_unsigned(Message::new(&instructions, Some(&user)));

        Ok(remove_liquidity_transaction)
    }

    // MANUAL HANDLING (these are prone to changes in the future)

    // before calling swap_ix/finalize_ix/add_liquidity_ix/remove_liquidity_ix -
    // load_pool has to be called at least once before and update_accounts before each function call

    /// Create a new Darklake AMM instance from account data
    pub async fn load_pool(
        &mut self,
        token_x: Pubkey,
        token_y: Pubkey,
    ) -> Result<(Pubkey, Pubkey, Pubkey)> {
        let (pool_key, _, _) = Self::get_pool_address(token_x, token_y);

        let pool_account_data = self
            .rpc_client
            .get_account(&pool_key)
            .await
            .map_err(|_| anyhow::anyhow!("Pool not found"))?;

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
        let accounts_to_update = self.darklake_amm.as_ref().unwrap().get_accounts_to_update();
        let mut account_map = HashMap::new();
        for account_key in accounts_to_update {
            let account = self.rpc_client.get_account(&account_key).await?;
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
    pub async fn get_order(
        &mut self,
        user: Pubkey,
        commitment_level: CommitmentLevel,
    ) -> Result<Order> {
        let order_key = self.darklake_amm.as_ref().unwrap().get_order_pubkey(user)?;

        let order_data = self
            .rpc_client
            .get_account_with_commitment(
                &order_key,
                CommitmentConfig {
                    commitment: commitment_level,
                },
            )
            .await?
            .value;
        if order_data.is_none() {
            return Err(anyhow::anyhow!("Order not found"));
        }

        let order_data = order_data.unwrap();

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
            .get_finalize_and_account_metas(
                &finalize_params,
                &ProofParams {
                    paths: self.settle_paths.clone(),
                },
                &ProofParams {
                    paths: self.cancel_paths.clone(),
                },
            )?;

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
