use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use spl_token::native_mint;

use crate::{
    amm::{
        AccountData, AddLiquidityParams, Amm, CancelParams, FinalizeParams, InitializePoolParams,
        KeyedAccount, ProofCircuitPaths, ProofParams, Quote, QuoteParams, RemoveLiquidityParams,
        SettleParams, SlashParams, SwapMode, SwapParams,
    },
    constants::{DARKLAKE_PROGRAM_ID, SOL_MINT},
    darklake_amm::{AmmConfig, DarklakeAmm, Order, Pool},
    proof::proof_generator::find_circuit_path,
    reduced_amm_params::{
        AddLiquidityParamsIx, FinalizeParamsIx, InitializePoolParamsIx, RemoveLiquidityParamsIx,
        SwapParamsIx,
    },
    utils::{
        convert_string_to_bytes_array, generate_random_salt, get_address_lookup_table,
        get_close_wsol_instructions, get_wrap_sol_to_wsol_instructions,
    },
};
use anyhow::{Context, Result};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    message::{VersionedMessage, v0},
    pubkey::Pubkey,
    transaction::VersionedTransaction,
};
use std::collections::HashMap;
use tokio::time::{Duration, sleep};

use crate::proof::proof_generator::{
    PrivateProofInputs, PublicProofInputs, convert_proof_to_solana_proof, from_32_byte_buffer,
    generate_proof,
};

pub struct DarklakeSDK {
    rpc_client: RpcClient,
    darklake_amm: DarklakeAmm,
    settle_paths: ProofCircuitPaths,
    cancel_paths: ProofCircuitPaths,
    is_devnet: bool, // supports only devnet or mainnet
    label: Option<[u8; 21]>,
    ref_code: Option<[u8; 20]>,
}

impl DarklakeSDK {
    /// Create a new Darklake SDK instance
    pub fn new(
        rpc_endpoint: &str,
        commitment_level: CommitmentLevel,
        is_devnet: bool, // only used for pool initialization
        label: Option<&str>,
        ref_code: Option<&str>,
    ) -> Result<Self> {
        let commitment_config = CommitmentConfig {
            commitment: commitment_level,
        };

        // label
        let sdk_label_prefix = "cv0.3.2";

        // sanity check for in-case we exceed prefix length
        if sdk_label_prefix.len() > 10 {
            return Err(anyhow::anyhow!(
                "SDK label prefix is too long, must be equal or less than 10 bytes"
            ));
        }

        let full_label = if label.is_some() {
            if label.unwrap().len() > 10 {
                return Err(anyhow::anyhow!(
                    "Label is too long, must be equal or less than 10 characters"
                ));
            }

            let label = label.unwrap();
            let joined_label = [sdk_label_prefix, label].join(",");
            convert_string_to_bytes_array(&joined_label, 21)?
        } else {
            convert_string_to_bytes_array(sdk_label_prefix, 21)?
        };

        let full_label_bytes: [u8; 21] = full_label
            .try_into()
            .map_err(|_| anyhow::anyhow!("Failed to convert final label to bytes array"))?;

        // ref code
        let ref_code_bytes: Option<[u8; 20]> = if let Some(ref_code) = ref_code {
            let ref_code_vec = convert_string_to_bytes_array(ref_code, 20)?;
            Some(
                ref_code_vec
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("Ref code failed to convert to bytes array"))?,
            )
        } else {
            None
        };

        let settle_file_prefix = "settle";
        let cancel_file_prefix = "cancel";

        let settle_wasm_path = find_circuit_path(&format!("{}.wasm", settle_file_prefix));
        let settle_zkey_path = find_circuit_path(&format!("{}_final.zkey", settle_file_prefix));
        let settle_r1cs_path = find_circuit_path(&format!("{}.r1cs", settle_file_prefix));

        let cancel_wasm_path = find_circuit_path(&format!("{}.wasm", cancel_file_prefix));
        let cancel_zkey_path = find_circuit_path(&format!("{}_final.zkey", cancel_file_prefix));
        let cancel_r1cs_path = find_circuit_path(&format!("{}.r1cs", cancel_file_prefix));

        Ok(Self {
            rpc_client: RpcClient::new_with_commitment(rpc_endpoint.to_string(), commitment_config),
            darklake_amm: DarklakeAmm {
                key: Pubkey::default(),
                pool: Pool::default(),
                amm_config: AmmConfig::default(),
                reserve_x_balance: 0,
                reserve_y_balance: 0,
                token_x_owner: Pubkey::default(),
                token_y_owner: Pubkey::default(),
                token_x_transfer_fee_config: None,
                token_y_transfer_fee_config: None,
            },
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
            is_devnet,
            label: Some(full_label_bytes),
            ref_code: ref_code_bytes,
        })
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
        token_in: &Pubkey,
        token_out: &Pubkey,
        amount_in: u64,
    ) -> Result<Quote> {
        let is_from_sol = *token_in == SOL_MINT;
        let is_to_sol = *token_out == SOL_MINT;

        let _token_in = if is_from_sol {
            native_mint::ID
        } else {
            token_in.clone()
        };
        let _token_out = if is_to_sol {
            native_mint::ID
        } else {
            token_out.clone()
        };

        let (pool_key, _token_x, _token_y) = Self::get_pool_address(&_token_in, &_token_out);

        if self.darklake_amm.key() != pool_key {
            self.load_pool(&_token_x, &_token_y).await?;
        }

        self.update_accounts().await?;

        let epoch = self.rpc_client.get_epoch_info().await?.epoch;

        self.darklake_amm.quote(&QuoteParams {
            input_mint: _token_in,
            amount: amount_in,
            swap_mode: SwapMode::ExactIn,
            epoch,
        })
    }

    /// Start a swap
    ///
    /// # Arguments
    /// * `token_in` - The input token mint
    /// * `token_out` - The output token mint
    /// * `amount_in` - The amount of input tokens
    /// * `min_out` - The minimum amount of output tokens expected
    /// * `token_owner` - The token owner public key
    ///
    /// # Returns
    /// Returns a `VersionedTransaction`, the order key, the minimum amount of output tokens expected (min_out), and the salt used
    pub async fn swap_tx(
        &mut self,
        token_in: &Pubkey,
        token_out: &Pubkey,
        amount_in: u64,
        min_out: u64,
        token_owner: &Pubkey,
    ) -> Result<(VersionedTransaction, Pubkey, u64, [u8; 8])> {
        let is_from_sol = *token_in == SOL_MINT;
        let is_to_sol = *token_out == SOL_MINT;

        let _token_in = if is_from_sol {
            native_mint::ID
        } else {
            token_in.clone()
        };
        let _token_out = if is_to_sol {
            native_mint::ID
        } else {
            token_out.clone()
        };

        let (pool_key, _token_x, _token_y) = Self::get_pool_address(&_token_in, &_token_out);

        if self.darklake_amm.key() != pool_key {
            self.load_pool(&_token_x, &_token_y).await?;
        }

        self.update_accounts().await?;

        let salt = generate_random_salt();

        let swap_params = SwapParamsIx {
            source_mint: _token_in,
            destination_mint: _token_out,
            token_transfer_authority: token_owner.clone(),
            amount_in,
            swap_mode: SwapMode::ExactIn,
            min_out,
            salt,
        };

        let swap_instruction = self.swap_ix(&swap_params).await?;

        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(300_000);

        let mut instructions = vec![compute_budget_ix];

        let address_lookup_table_account =
            get_address_lookup_table(&self.rpc_client, self.is_devnet).await?;

        if is_from_sol {
            let sol_to_wsol_instructions =
                get_wrap_sol_to_wsol_instructions(&token_owner, amount_in)?;
            instructions.push(sol_to_wsol_instructions[0].clone());
            instructions.push(sol_to_wsol_instructions[1].clone());
            instructions.push(sol_to_wsol_instructions[2].clone());
        }

        instructions.push(swap_instruction);

        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        let message_v0 = v0::Message::try_compile(
            &token_owner,
            &instructions,
            &[address_lookup_table_account],
            recent_blockhash,
        )?;

        let swap_transaction = VersionedTransaction {
            signatures: vec![],
            message: VersionedMessage::V0(message_v0),
        };

        let order_key = self.darklake_amm.get_order_pubkey(&token_owner)?;

        Ok((swap_transaction, order_key, min_out, salt))
    }

    /// Finalize a swap order by settling, canceling, or slashing it
    ///
    /// # Arguments
    /// * `order_key` - The public key of the order to finalize
    /// * `unwrap_wsol` - Whether to unwrap WSOL to SOL after settlement
    /// * `min_out` - The minimum output amount expected (same as swap)
    /// * `salt` - The salt used in the original swap (same as swap)
    /// * `settle_signer` - Optional signer for settlement (defaults to order owner)
    ///
    /// # Returns
    /// Returns a `VersionedTransaction` ready to be signed and sent
    pub async fn finalize_tx(
        &mut self,
        order_key: &Pubkey,
        unwrap_wsol: bool,
        min_out: u64,
        salt: [u8; 8],
        settle_signer: Option<&Pubkey>,
    ) -> Result<VersionedTransaction> {
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
            .parse_order_data(&order_data.unwrap().data)?;

        self.update_accounts().await?;

        let settler = settle_signer.unwrap_or(&order.trader);
        let create_wsol_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &settler,
                &settler,
                &native_mint::ID,
                &spl_token::ID,
            );

        let finalize_params = FinalizeParamsIx {
            settle_signer: settler.clone(), // who settles the order
            order_owner: order.trader,      // who owns the order
            unwrap_wsol,                    // Set to true if you want to unwrap WSOL to SOL
            min_out,                        // Same min_out as swap
            salt,                           // Same salt as swap
            output: order.d_out,            // order prop
            commitment: order.c_min,        // order prop
            deadline: order.deadline,       // order prop
            current_slot: self
                .rpc_client
                .get_slot_with_commitment(CommitmentConfig::processed())
                .await?,
        };

        let finalize_instruction = self.finalize_ix(&finalize_params).await?;

        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(500_000);

        let instructions = vec![compute_budget_ix, create_wsol_ata_ix, finalize_instruction];

        let address_lookup_table_account =
            get_address_lookup_table(&self.rpc_client, self.is_devnet).await?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        let message_v0 = v0::Message::try_compile(
            &settler,
            &instructions,
            &[address_lookup_table_account],
            recent_blockhash,
        )?;

        let finalize_transaction = VersionedTransaction {
            signatures: vec![],
            message: VersionedMessage::V0(message_v0),
        };

        Ok(finalize_transaction)
    }

    /// Add liquidity to a pool
    ///
    /// # Arguments
    /// * `token_x` - The first token mint address
    /// * `token_y` - The second token mint address
    /// * `max_amount_x` - Maximum amount of token_x to add
    /// * `max_amount_y` - Maximum amount of token_y to add
    /// * `amount_lp` - Amount of LP tokens to mint
    /// * `user` - The user's public key
    ///
    /// # Returns
    /// Returns a `VersionedTransaction` ready to be signed and sent
    pub async fn add_liquidity_tx(
        &mut self,
        token_x: &Pubkey,
        token_y: &Pubkey,
        max_amount_x: u64,
        max_amount_y: u64,
        amount_lp: u64,
        user: &Pubkey,
    ) -> Result<VersionedTransaction> {
        let is_x_sol = *token_x == SOL_MINT;
        let is_y_sol = *token_y == SOL_MINT;

        let token_x_post_sol = if is_x_sol {
            native_mint::ID
        } else {
            token_x.clone()
        };
        let token_y_post_sol = if is_y_sol {
            native_mint::ID
        } else {
            token_y.clone()
        };

        let (pool_key, _token_x, _token_y) =
            Self::get_pool_address(&token_x_post_sol, &token_y_post_sol);

        // swaps ammount of token_x and token_y if tokens are not sorted
        let (max_amount_x, max_amount_y) = if _token_x != token_x_post_sol {
            (max_amount_y, max_amount_x)
        } else {
            (max_amount_x, max_amount_y)
        };

        if self.darklake_amm.key() != pool_key {
            self.load_pool(&_token_x, &_token_y).await?;
        }

        self.update_accounts().await?;

        let add_liquidity_params = AddLiquidityParamsIx {
            amount_lp,
            max_amount_x,
            max_amount_y,
            user: user.clone(),
        };

        let add_liquidity_instruction = self.add_liquidity_ix(&add_liquidity_params).await?;

        let mut instructions = vec![];
        if is_x_sol {
            let sol_to_wsol_instructions = get_wrap_sol_to_wsol_instructions(&user, max_amount_x)?;
            instructions.push(sol_to_wsol_instructions[0].clone());
            instructions.push(sol_to_wsol_instructions[1].clone());
            instructions.push(sol_to_wsol_instructions[2].clone());
        } else if is_y_sol {
            let sol_to_wsol_instructions = get_wrap_sol_to_wsol_instructions(&user, max_amount_y)?;
            instructions.push(sol_to_wsol_instructions[0].clone());
            instructions.push(sol_to_wsol_instructions[1].clone());
            instructions.push(sol_to_wsol_instructions[2].clone());
        }

        instructions.push(add_liquidity_instruction);

        let address_lookup_table_account =
            get_address_lookup_table(&self.rpc_client, self.is_devnet).await?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        let message_v0 = v0::Message::try_compile(
            &user,
            &instructions,
            &[address_lookup_table_account],
            recent_blockhash,
        )?;

        let add_liquidity_transaction = VersionedTransaction {
            signatures: vec![],
            message: VersionedMessage::V0(message_v0),
        };

        Ok(add_liquidity_transaction)
    }

    /// Remove liquidity from a pool
    ///
    /// # Arguments
    /// * `token_x` - The first token mint address
    /// * `token_y` - The second token mint address
    /// * `min_amount_x` - Minimum amount of token_x to receive
    /// * `min_amount_y` - Minimum amount of token_y to receive
    /// * `amount_lp` - Amount of LP tokens to burn
    /// * `user` - The user's public key
    ///
    /// # Returns
    /// Returns a `VersionedTransaction` ready to be signed and sent
    pub async fn remove_liquidity_tx(
        &mut self,
        token_x: &Pubkey,
        token_y: &Pubkey,
        min_amount_x: u64,
        min_amount_y: u64,
        amount_lp: u64,
        user: &Pubkey,
    ) -> Result<VersionedTransaction> {
        let is_x_sol = *token_x == SOL_MINT;
        let is_y_sol = *token_y == SOL_MINT;

        let token_x_post_sol = if is_x_sol {
            native_mint::ID
        } else {
            token_x.clone()
        };
        let token_y_post_sol = if is_y_sol {
            native_mint::ID
        } else {
            token_y.clone()
        };

        let (pool_key, _token_x, _token_y) =
            Self::get_pool_address(&token_x_post_sol, &token_y_post_sol);

        let (min_amount_x, min_amount_y) = if _token_x != token_x_post_sol {
            (min_amount_y, min_amount_x)
        } else {
            (min_amount_x, min_amount_y)
        };

        if self.darklake_amm.key() != pool_key {
            self.load_pool(&_token_x, &_token_y).await?;
        }

        self.update_accounts().await?;

        let (token_x_owner, token_y_owner) = self.darklake_amm.get_token_owners();

        // make sure the user has the token accounts
        let create_token_x_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                user,
                user,
                &_token_x,
                &token_x_owner,
            );

        let create_token_y_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                user,
                user,
                &_token_y,
                &token_y_owner,
            );

        let remove_liquidity_params = RemoveLiquidityParamsIx {
            amount_lp,
            min_amount_x,
            min_amount_y,
            user: user.clone(),
        };

        let remove_liquidity_instruction =
            self.remove_liquidity_ix(&remove_liquidity_params).await?;

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

        let address_lookup_table_account =
            get_address_lookup_table(&self.rpc_client, self.is_devnet).await?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        let message_v0 = v0::Message::try_compile(
            &user,
            &instructions,
            &[address_lookup_table_account],
            recent_blockhash,
        )?;

        let remove_liquidity_transaction = VersionedTransaction {
            signatures: vec![],
            message: VersionedMessage::V0(message_v0),
        };

        Ok(remove_liquidity_transaction)
    }

    /// Initialize a new liquidity pool
    ///
    /// # Arguments
    /// * `token_x` - The first token mint address
    /// * `token_y` - The second token mint address
    /// * `amount_x` - Initial amount of token_x to add
    /// * `amount_y` - Initial amount of token_y to add
    /// * `user` - The user's public key
    ///
    /// # Returns
    /// Returns a `VersionedTransaction` ready to be signed and sent
    pub async fn initialize_pool_tx(
        &mut self,
        token_x: &Pubkey,
        token_y: &Pubkey,
        amount_x: u64,
        amount_y: u64,
        user: &Pubkey,
    ) -> Result<VersionedTransaction> {
        let is_x_sol = *token_x == SOL_MINT;
        let is_y_sol = *token_y == SOL_MINT;

        let token_x_post_sol = if is_x_sol {
            native_mint::ID
        } else {
            token_x.clone()
        };
        let token_y_post_sol = if is_y_sol {
            native_mint::ID
        } else {
            token_y.clone()
        };

        // used to sort token mints
        let (_pool_key, _token_x, _token_y) =
            Self::get_pool_address(&token_x_post_sol, &token_y_post_sol);

        let (amount_x, amount_y) = if _token_x != token_x_post_sol {
            (amount_y, amount_x)
        } else {
            (amount_x, amount_y)
        };

        let token_x_account = self.rpc_client.get_account(&_token_x).await?;
        let token_y_account = self.rpc_client.get_account(&_token_y).await?;

        let initialize_pool_params = InitializePoolParamsIx {
            user: user.clone(),
            token_x: _token_x,
            token_x_program: token_x_account.owner,
            token_y: _token_y,
            token_y_program: token_y_account.owner,
            amount_x,
            amount_y,
        };

        let compute_budget_ix: Instruction =
            ComputeBudgetInstruction::set_compute_unit_limit(500_000);

        let initialize_pool_instruction = self.initialize_pool_ix(&initialize_pool_params).await?;

        let mut instructions = vec![compute_budget_ix];
        if is_x_sol {
            let sol_to_wsol_instructions = get_wrap_sol_to_wsol_instructions(user, amount_x)?;
            instructions.push(sol_to_wsol_instructions[0].clone());
            instructions.push(sol_to_wsol_instructions[1].clone());
            instructions.push(sol_to_wsol_instructions[2].clone());
        } else if is_y_sol {
            let sol_to_wsol_instructions = get_wrap_sol_to_wsol_instructions(user, amount_y)?;
            instructions.push(sol_to_wsol_instructions[0].clone());
            instructions.push(sol_to_wsol_instructions[1].clone());
            instructions.push(sol_to_wsol_instructions[2].clone());
        }

        instructions.push(initialize_pool_instruction);

        let address_lookup_table_account =
            get_address_lookup_table(&self.rpc_client, self.is_devnet).await?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        let message_v0 = v0::Message::try_compile(
            &user,
            &instructions,
            &[address_lookup_table_account],
            recent_blockhash,
        )?;

        let initialize_pool_transaction = VersionedTransaction {
            signatures: vec![],
            message: VersionedMessage::V0(message_v0),
        };

        Ok(initialize_pool_transaction)
    }

    // MANUAL HANDLING (these are prone to changes in the future)

    // before calling swap_ix/finalize_ix/add_liquidity_ix/remove_liquidity_ix -
    // load_pool has to be called at least once before usage and update_accounts before each function call

    /// Load pool data from the blockchain
    ///
    /// # Arguments
    /// * `token_x` - The first token mint address
    /// * `token_y` - The second token mint address
    ///
    /// # Returns
    /// Returns a tuple of (pool_key, sorted_token_x, sorted_token_y)
    pub async fn load_pool(
        &mut self,
        token_x: &Pubkey,
        token_y: &Pubkey,
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

        self.darklake_amm = DarklakeAmm::load_pool(&pool_key_and_account)?;

        // returns sorted token mints
        Ok((pool_key, token_x.clone(), token_y.clone()))
    }

    /// Update account data from the blockchain
    ///
    /// This function fetches the latest account data for all accounts that need to be updated
    /// and updates the internal AMM state accordingly.
    ///
    /// # Returns
    /// Returns `Ok(())` on success
    pub async fn update_accounts(&mut self) -> Result<()> {
        let accounts_to_update = self.darklake_amm.get_accounts_to_update();
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
        self.darklake_amm.update(&account_map)?;

        Ok(())
    }

    /// Get order data for a user
    ///
    /// This function does not require load_pool or update_accounts and is a standalone function
    /// that can be called after new() is called.
    ///
    /// # Arguments
    /// * `user` - The user's public key
    /// * `commitment_level` - The commitment level for the RPC call
    ///
    /// # Returns
    /// Returns the `Order` data for the user
    pub async fn get_order(
        &self,
        user: &Pubkey,
        commitment_level: CommitmentLevel,
    ) -> Result<Order> {
        let order_key = self.darklake_amm.get_order_pubkey(user)?;

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

        let order = self.darklake_amm.parse_order_data(&order_data.data)?;

        Ok(order)
    }

    /// Create a swap instruction
    ///
    /// # Arguments
    /// * `swap_params` - The swap parameters
    ///
    /// # Returns
    /// Returns a `Instruction` ready to be added to a transaction
    pub async fn swap_ix(&self, swap_params: &SwapParamsIx) -> Result<Instruction> {
        let swap_params = SwapParams {
            source_mint: swap_params.source_mint,
            destination_mint: swap_params.destination_mint,
            token_transfer_authority: swap_params.token_transfer_authority,
            amount_in: swap_params.amount_in,
            swap_mode: swap_params.swap_mode,
            min_out: swap_params.min_out,
            salt: swap_params.salt,
            label: self.label,
        };

        let swap_and_account_metas = self
            .darklake_amm
            .get_swap_and_account_metas(&swap_params)
            .context("Failed to get swap instruction and account metadata")?;

        Ok(Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: swap_and_account_metas.account_metas,
            data: swap_and_account_metas.data,
        })
    }

    /// Create a finalize instruction (settle or cancel)
    ///
    /// # Arguments
    /// * `finalize_params` - The finalize parameters
    ///
    /// # Returns
    /// Returns a `Instruction` ready to be added to a transaction
    pub async fn finalize_ix(&self, finalize_params: &FinalizeParamsIx) -> Result<Instruction> {
        let finalize_params = FinalizeParams {
            settle_signer: finalize_params.settle_signer,
            order_owner: finalize_params.order_owner,
            unwrap_wsol: finalize_params.unwrap_wsol,
            min_out: finalize_params.min_out,
            salt: finalize_params.salt,
            output: finalize_params.output,
            commitment: finalize_params.commitment,
            deadline: finalize_params.deadline,
            current_slot: finalize_params.current_slot,
            label: self.label,
            ref_code: self.ref_code,
        };

        let is_settle = finalize_params.min_out <= finalize_params.output;
        let is_slash = finalize_params.current_slot > finalize_params.deadline;

        if is_slash {
            let slash_and_account_metas =
                self.darklake_amm
                    .get_slash_and_account_metas(&SlashParams {
                        settle_signer: finalize_params.settle_signer,
                        order_owner: finalize_params.order_owner,
                        deadline: finalize_params.deadline,
                        current_slot: finalize_params.current_slot,
                        label: finalize_params.label,
                    })?;
            return Ok(Instruction {
                program_id: DARKLAKE_PROGRAM_ID,
                accounts: slash_and_account_metas.account_metas,
                data: slash_and_account_metas.data,
            });
        }

        let circuit_paths = if is_settle {
            self.settle_paths.clone()
        } else {
            self.cancel_paths.clone()
        };

        let private_inputs = PrivateProofInputs {
            min_out: finalize_params.min_out,
            salt: u64::from_le_bytes(finalize_params.salt),
        };

        let public_inputs = PublicProofInputs {
            real_out: finalize_params.output,
            commitment: from_32_byte_buffer(&finalize_params.commitment),
        };

        let (proof, _) = generate_proof(
            &private_inputs,
            &public_inputs,
            &circuit_paths.wasm_path,
            &circuit_paths.zkey_path,
            &circuit_paths.r1cs_path,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to generate proof: {}", e))?;

        let solana_proof = convert_proof_to_solana_proof(&proof, &public_inputs);
        let public_inputs_vec = solana_proof.public_signals.clone();
        let public_inputs_arr: [[u8; 32]; 2] = public_inputs_vec
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid public signals length"))?;

        if is_settle {
            let settle_params = SettleParams {
                settle_signer: finalize_params.settle_signer,
                order_owner: finalize_params.order_owner,
                unwrap_wsol: finalize_params.unwrap_wsol,
                min_out: finalize_params.min_out,
                salt: finalize_params.salt,
                output: finalize_params.output,
                commitment: finalize_params.commitment,
                deadline: finalize_params.deadline,
                current_slot: finalize_params.current_slot,
                ref_code: finalize_params.ref_code,
                label: finalize_params.label,
            };
            let settle_and_account_metas = self.darklake_amm.get_settle_and_account_metas(
                &settle_params,
                &ProofParams {
                    generated_proof: solana_proof,
                    public_inputs: public_inputs_arr,
                },
            )?;

            return Ok(Instruction {
                program_id: DARKLAKE_PROGRAM_ID,
                accounts: settle_and_account_metas.account_metas,
                data: settle_and_account_metas.data,
            });
        }

        let cancel_params = CancelParams {
            settle_signer: finalize_params.settle_signer,
            order_owner: finalize_params.order_owner,
            min_out: finalize_params.min_out,
            salt: finalize_params.salt,
            output: finalize_params.output,
            commitment: finalize_params.commitment,
            deadline: finalize_params.deadline,
            current_slot: finalize_params.current_slot,
            label: finalize_params.label,
        };
        let cancel_and_account_metas = self.darklake_amm.get_cancel_and_account_metas(
            &cancel_params,
            &ProofParams {
                generated_proof: solana_proof,
                public_inputs: public_inputs_arr,
            },
        )?;

        return Ok(Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: cancel_and_account_metas.account_metas,
            data: cancel_and_account_metas.data,
        });
    }

    /// Create an add liquidity instruction
    ///
    /// # Arguments
    /// * `add_liquidity_params` - The add liquidity parameters
    ///
    /// # Returns
    /// Returns a `Instruction` ready to be added to a transaction
    pub async fn add_liquidity_ix(
        &self,
        add_liquidity_params: &AddLiquidityParamsIx,
    ) -> Result<Instruction> {
        let add_liquidity_params = AddLiquidityParams {
            amount_lp: add_liquidity_params.amount_lp,
            max_amount_x: add_liquidity_params.max_amount_x,
            max_amount_y: add_liquidity_params.max_amount_y,
            user: add_liquidity_params.user,
            label: self.label,
            ref_code: self.ref_code,
        };

        let add_liquidity_and_account_metas = self
            .darklake_amm
            .get_add_liquidity_and_account_metas(&add_liquidity_params)?;

        Ok(Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: add_liquidity_and_account_metas.account_metas,
            data: add_liquidity_and_account_metas.data,
        })
    }

    /// Create a remove liquidity instruction
    ///
    /// # Arguments
    /// * `remove_liquidity_params` - The remove liquidity parameters
    ///
    /// # Returns
    /// Returns a `Instruction` ready to be added to a transaction
    pub async fn remove_liquidity_ix(
        &self,
        remove_liquidity_params: &RemoveLiquidityParamsIx,
    ) -> Result<Instruction> {
        let remove_liquidity_params = RemoveLiquidityParams {
            amount_lp: remove_liquidity_params.amount_lp,
            min_amount_x: remove_liquidity_params.min_amount_x,
            min_amount_y: remove_liquidity_params.min_amount_y,
            user: remove_liquidity_params.user,
            label: self.label,
        };

        let remove_liquidity_and_account_metas = self
            .darklake_amm
            .get_remove_liquidity_and_account_metas(&remove_liquidity_params)?;

        Ok(Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: remove_liquidity_and_account_metas.account_metas,
            data: remove_liquidity_and_account_metas.data,
        })
    }

    /// Create an initialize pool instruction
    ///
    /// # Arguments
    /// * `initialize_pool_params` - The initialize pool parameters
    ///
    /// # Returns
    /// Returns a `Instruction` ready to be added to a transaction
    pub async fn initialize_pool_ix(
        &self,
        initialize_pool_params: &InitializePoolParamsIx,
    ) -> Result<Instruction> {
        let initialize_pool_params = InitializePoolParams {
            user: initialize_pool_params.user,
            token_x: initialize_pool_params.token_x,
            token_x_program: initialize_pool_params.token_x_program,
            token_y: initialize_pool_params.token_y,
            token_y_program: initialize_pool_params.token_y_program,
            amount_x: initialize_pool_params.amount_x,
            amount_y: initialize_pool_params.amount_y,
            label: self.label,
        };

        let initialize_pool_and_account_metas = self
            .darklake_amm
            .get_initialize_pool_and_account_metas(&initialize_pool_params, self.is_devnet)?;

        Ok(Instruction {
            program_id: DARKLAKE_PROGRAM_ID,
            accounts: initialize_pool_and_account_metas.account_metas,
            data: initialize_pool_and_account_metas.data,
        })
    }

    /// Helpers internal methods
    /// Get the pool address for a token pair
    fn get_pool_address(token_mint_x: &Pubkey, token_mint_y: &Pubkey) -> (Pubkey, Pubkey, Pubkey) {
        let (ordered_x, ordered_y) = if token_mint_x < token_mint_y {
            (token_mint_x, token_mint_y)
        } else {
            (token_mint_y, token_mint_x)
        };

        let pool_key = crate::darklake_amm::DarklakeAmm::get_pool_address(ordered_x, ordered_y);

        (pool_key, ordered_x.clone(), ordered_y.clone())
    }
}
