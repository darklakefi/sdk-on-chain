use anchor_lang::prelude::*;

use crate::amm::*;

use crate::math::{get_transfer_fee, swap, rebalance_pool_ratio, SwapResultWithFromToLock};
use crate::proof::proof_generator::{convert_proof_to_solana_proof, from_32_byte_buffer, generate_proof, to_32_byte_buffer, PrivateProofInputs, PublicProofInputs};
use crate::proof::utils::{bytes_to_bigint, compute_poseidon_hash_with_salt, u64_array_to_u8_array_le};

use anchor_lang::{system_program, AnchorDeserialize, AnchorSerialize};
use solana_sdk::{program_pack::Pack, pubkey, pubkey::Pubkey};
use rust_decimal::Decimal;
use anyhow::{Result, bail, Context};
use spl_token::{native_mint, state::Account as SplTokenAccount};
use spl_token_2022::extension::{transfer_fee::TransferFeeConfig, BaseStateWithExtensions, StateWithExtensions};

#[derive(Clone)]
pub struct DarklakeAmm {
    pub key: Pubkey,
    pub pool: Pool,
    pub amm_config: AmmConfig,
    pub reserve_x_balance: u64,
    pub reserve_y_balance: u64,
    pub token_x_owner: Pubkey,
    pub token_y_owner: Pubkey,
    pub token_x_transfer_fee_config: Option<TransferFeeConfig>,
    pub token_y_transfer_fee_config: Option<TransferFeeConfig>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AmmConfig {
    pub trade_fee_rate: u64,    // 10^6 = 100%
    pub create_pool_fee: u64,   // flat SOL fee for creating a pool
    pub protocol_fee_rate: u64, // 10^6 = 100% (precentage of trade fee)

    pub wsol_trade_deposit: u64, // this should AT LEAST be the size of tx fee + any account creation fees

    pub deadline_slot_duration: u64,

    pub ratio_change_tolerance_rate: u64, // 10^6 = 100%

    pub bump: u8,
    pub halted: bool, // if true, no actions are allowed

    /// padding
    pub padding: [u64; 16],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Order {
    // pubkeys
    pub trader: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,

    // quantities
    pub actual_in: u64,    // amount taken from user
    pub exchange_in: u64,  // amount received by the pool (post token fees)
    pub actual_out: u64,   // amount received by user
    pub from_to_lock: u64, // amount locked in the pool
    pub d_in: u64,         // locked_x
    pub d_out: u64,        // locked_y
    pub deadline: u64,
    pub protocol_fee: u64,
    pub wsol_deposit: u64,

    // proof
    pub c_min: [u8; 32],

    pub is_x_to_y: bool,
    pub bump: u8,

    pub padding: [u64; 4],
}


#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Pool {
    // pubkeys
    pub creator: Pubkey,
    pub amm_config: Pubkey,

    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,

    pub reserve_x: Pubkey,
    pub reserve_y: Pubkey,

    // quanities
    pub token_lp_supply: u64,
    pub protocol_fee_x: u64,
    pub protocol_fee_y: u64,

    // locked for existing orders
    pub locked_x: u64,
    pub locked_y: u64,

    pub user_locked_x: u64,
    pub user_locked_y: u64,

    pub bump: u8,

    pub padding: [u64; 4],
}

pub const DARKLAKE_PROGRAM_ID: Pubkey = pubkey!("darkr3FB87qAZmgLwKov6Hk9Yiah5UT4rUYu8Zhthw1");

impl Amm for DarklakeAmm {
    fn label(&self) -> String {
        "Darklake".to_string()
    }

    fn from_keyed_account(keyed_account: &KeyedAccount) -> Result<Self> 
    where
        Self: Sized {
            Ok(DarklakeAmm {
                key: keyed_account.key,
                pool: Pool::deserialize(&mut &keyed_account.account.data[8..])?,
                amm_config: AmmConfig {
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
                token_x_owner: Pubkey::default(),
                token_y_owner: Pubkey::default(),
                token_x_transfer_fee_config: None,
                token_y_transfer_fee_config: None,
            })
        }

    fn program_id(&self) -> Pubkey {
        DARKLAKE_PROGRAM_ID
    }
    
    fn key(&self) -> Pubkey {
        self.key
    }
    
    fn get_reserve_mints(&self) -> Vec<Pubkey> {
        vec![self.pool.token_mint_x, self.pool.token_mint_y]
    }
    
    fn get_accounts_to_update(&self) -> Vec<Pubkey> {
        vec![
            self.key,
            self.pool.token_mint_x, // tokens
            self.pool.token_mint_y,
            self.pool.reserve_x,    // pool token reserves
            self.pool.reserve_y,
            self.pool.amm_config,   // config with fee values
        ]
    }
    
    fn update(&mut self, account_map: &AccountMap) -> Result<()> {
        let account = account_map.get(&self.key).context("Darklake pool account not found")?;

        self.pool = Pool::deserialize(&mut &account.data[8..])?;

        let amm_config_data = try_get_account_data(account_map, &self.pool.amm_config)?;
        let amm_config = AmmConfig::deserialize(&mut &amm_config_data[8..])?;

        self.amm_config = amm_config;
        
        let (token_x_data, token_x_owner) = try_get_account_data_and_owner(account_map, &self.pool.reserve_x)?;
        let (token_y_data, token_y_owner) = try_get_account_data_and_owner(account_map, &self.pool.reserve_y)?;

        // Parse token account balances reliably for both SPL and Token2022 tokens
        self.reserve_x_balance = Self::parse_token_account_balance(&token_x_data, &token_x_owner, &self.pool.reserve_x)?;
        self.reserve_y_balance = Self::parse_token_account_balance(&token_y_data, &token_y_owner, &self.pool.reserve_y)?;

        self.token_x_transfer_fee_config = self.get_transfer_fee_config(&token_x_data, token_x_owner).unwrap_or(None);
        self.token_y_transfer_fee_config = self.get_transfer_fee_config(&token_y_data, token_y_owner).unwrap_or(None);

        self.token_x_owner = *token_x_owner;
        self.token_y_owner = *token_y_owner;

        Ok(())
    }

    fn supports_exact_out(&self) -> bool {
        false
    }
    
    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        let swap_mode = quote_params.swap_mode;
        if swap_mode != SwapMode::ExactIn {
            bail!("Exact out not supported");
        }

        self.quote_internal(quote_params, &self.amm_config, self.reserve_x_balance, self.reserve_y_balance)
    }

    fn get_swap_and_account_metas(&self, swap_params: &SwapParams) -> Result<SwapAndAccountMetas> {
        let SwapParams {
            token_transfer_authority,
            salt,
            min_out,
            ..
        } = swap_params;

        let authority = self.get_authority();
        let is_swap_x_to_y = swap_params.source_mint == self.pool.token_mint_x;

        let user_token_account_wsol = self.get_user_token_account(*token_transfer_authority, native_mint::ID, spl_token::ID);
        let user_token_account_x = self.get_user_token_account(*token_transfer_authority, self.pool.token_mint_x, self.token_x_owner);
        let user_token_account_y = self.get_user_token_account(*token_transfer_authority, self.pool.token_mint_y, self.token_y_owner);

        let pool_wsol_reserve = self.get_pool_wsol_reserve();
        let order = self.get_order(*token_transfer_authority);
        
        // ADD C_MIN COMMITMENT CALCULATION HERE
        
        let commitment = to_32_byte_buffer(&bytes_to_bigint(&u64_array_to_u8_array_le(
            &compute_poseidon_hash_with_salt(*min_out, *salt),
        )));
        let discriminator = [248, 198, 158, 145, 225, 117, 135, 200];

        let mut data = discriminator.to_vec();

        data.extend_from_slice(&swap_params.in_amount.to_le_bytes());
        data.extend_from_slice(&[is_swap_x_to_y as u8]);
        data.extend_from_slice(&commitment);

        Ok(SwapAndAccountMetas {
            discriminator,
            swap: DarklakeAmmSwapParams {
                amount_in: swap_params.in_amount,
                is_swap_x_to_y,
                c_min: commitment,
            },
            data,
            account_metas: 
                DarklakeAmmSwap {
                    user: *token_transfer_authority,
                    token_mint_x: self.pool.token_mint_x,
                    token_mint_y: self.pool.token_mint_y,
                    token_mint_wsol: native_mint::ID,
                    pool: self.key,
                    authority,
                    amm_config: self.pool.amm_config,
                    user_token_account_x,
                    user_token_account_y,
                    user_token_account_wsol,
                    pool_token_reserve_x: self.pool.reserve_x,
                    pool_token_reserve_y: self.pool.reserve_y,
                    pool_wsol_reserve,
                    order,
                    associated_token_program: spl_associated_token_account::ID,
                    system_program: system_program::ID,
                    token_mint_x_program: self.token_x_owner,
                    token_mint_y_program: self.token_y_owner,
                    token_program: spl_token::ID,
                }
                .into()
        })
    }
    
    fn clone_amm(&self) -> Box<dyn Amm + Send + Sync> {
        Box::new(self.clone())
    }

    fn is_active(&self) -> bool {
        !self.amm_config.halted
    }

    // darklake specific settlement-related functions

    fn get_order_pubkey(&self, user: Pubkey) -> Result<Pubkey> {
        if self.key == Pubkey::default() {
            bail!("Darklake pool is not initialized");
        }
        Ok(self.get_order(user))
    }

    fn get_order_output_and_deadline(&self, order_data: &[u8]) -> Result<(u64, u64)> {
        let order = Order::deserialize(&mut &order_data[8..])?;
        Ok((order.actual_out, order.deadline))
    }

    fn is_order_expired(&self, order_data: &[u8], current_slot: u64) -> Result<bool> {
        let order = Order::deserialize(&mut &order_data[8..])?;
        Ok(order.deadline < current_slot)
    }

    // both for settle
    fn get_settle_and_account_metas(&self, settle_params: &SettleParams) -> Result<SettleAndAccountMetas> {
        let SettleParams {
            settle_signer,
            order_owner,
            unwrap_wsol,
            min_out,
            salt,
            output,
            commitment,
            deadline,
            current_slot,
        } = settle_params;

        if *current_slot > *deadline {
            bail!("Order has expired");
        }

        let authority = self.get_authority();

        let pool_wsol_reserve = self.get_pool_wsol_reserve();
        
        let user_token_account_wsol = self.get_user_token_account(*order_owner, native_mint::ID, spl_token::ID);
        let user_token_account_x = self.get_user_token_account(*order_owner, self.pool.token_mint_x, self.token_x_owner);
        let user_token_account_y = self.get_user_token_account(*order_owner, self.pool.token_mint_y, self.token_y_owner);

        let caller_token_account_wsol = self.get_user_token_account(*settle_signer, native_mint::ID, spl_token::ID);
        let order = self.get_order(*order_owner);
        let order_token_account_wsol = self.get_order_token_account_wsol(*order_owner);

        let private_inputs = PrivateProofInputs {
            min_out: *min_out,
            salt: u64::from_le_bytes(*salt),
        };
    
        let public_inputs = PublicProofInputs {
            real_out: *output,
            commitment: from_32_byte_buffer(&commitment),
        };
    
        // ADD PROOF CALCULATION HERE
        let (proof, _) =
            generate_proof(&private_inputs, &public_inputs, false)
                .map_err(|e| anyhow::anyhow!("Failed to generate proof: {}", e))?;

        let solana_proof = convert_proof_to_solana_proof(&proof, &public_inputs);
        let public_inputs_vec = solana_proof.public_signals.clone();
        let public_inputs_arr: [[u8; 32]; 2] = public_inputs_vec
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid public signals length"))?;

        // check if cancel or settle
        let is_settle = *min_out <= *output;

        if !is_settle {
            bail!("Cant settle this order, min_out > output");
        }
        let discriminator = [175, 42, 185, 87, 144, 131, 102, 212];

        let mut data = discriminator.to_vec();
        data.extend_from_slice(&solana_proof.proof_a);
        data.extend_from_slice(&solana_proof.proof_b);
        data.extend_from_slice(&solana_proof.proof_c);
        data.extend_from_slice(&public_inputs_arr[0]);
        data.extend_from_slice(&public_inputs_arr[1]);
        data.extend_from_slice(&[*unwrap_wsol as u8]);

        Ok(SettleAndAccountMetas {
            discriminator,
            settle: DarklakeAmmSettleParams {
                proof_a: solana_proof.proof_a,
                proof_b: solana_proof.proof_b,
                proof_c: solana_proof.proof_c,
                public_signals: public_inputs_arr,
                unwrap_wsol: *unwrap_wsol,
            },
            data,
            account_metas: 
                DarklakeAmmSettle {
                    caller: *settle_signer,
                    order_owner: *order_owner,
                    token_mint_x: self.pool.token_mint_x,
                    token_mint_y: self.pool.token_mint_y,
                    token_mint_wsol: native_mint::ID,
                    pool: self.key,
                    authority,
                    pool_token_reserve_x: self.pool.reserve_x,
                    pool_token_reserve_y: self.pool.reserve_y,
                    pool_wsol_reserve,
                    amm_config: self.pool.amm_config,
                    user_token_account_x,
                    user_token_account_y,
                    user_token_account_wsol,
                    caller_token_account_wsol,
                    order,
                    order_token_account_wsol,
                    system_program: system_program::ID,
                    associated_token_program: spl_associated_token_account::ID,
                    token_mint_x_program: self.token_x_owner,
                    token_mint_y_program: self.token_y_owner,
                    token_program: spl_token::ID,
                }
                .into()
        })
    }


    fn get_cancel_and_account_metas(&self, cancel_params: &CancelParams) -> Result<CancelAndAccountMetas> {
        let CancelParams {
            settle_signer,
            order_owner,
            min_out,
            salt,
            output,
            commitment,
            deadline,
            current_slot,
        } = cancel_params;

        if *current_slot > *deadline {
            bail!("Order has expired");
        }

        let authority = self.get_authority();

        let pool_wsol_reserve = self.get_pool_wsol_reserve();
        
        let user_token_account_wsol = self.get_user_token_account(*order_owner, native_mint::ID, spl_token::ID);
        let user_token_account_x = self.get_user_token_account(*order_owner, self.pool.token_mint_x, self.token_x_owner);
        let user_token_account_y = self.get_user_token_account(*order_owner, self.pool.token_mint_y, self.token_y_owner);

        let caller_token_account_wsol = self.get_user_token_account(*settle_signer, native_mint::ID, spl_token::ID);
        let order = self.get_order(*order_owner);

        let private_inputs = PrivateProofInputs {
            min_out: *min_out,
            salt: u64::from_le_bytes(*salt),
        };
    
        let public_inputs = PublicProofInputs {
            real_out: *output,
            commitment: from_32_byte_buffer(&commitment),
        };
    
        // ADD PROOF CALCULATION HERE
        let (proof, _) =
            generate_proof(&private_inputs, &public_inputs, true)
                .map_err(|e| anyhow::anyhow!("Failed to generate proof: {}", e))?;

        let solana_proof = convert_proof_to_solana_proof(&proof, &public_inputs);
        let public_inputs_vec = solana_proof.public_signals.clone();
        let public_inputs_arr: [[u8; 32]; 2] = public_inputs_vec
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid public signals length"))?;

        // check if cancel or settle
        let is_cancel = *min_out > *output;

        if !is_cancel {
            bail!("Cant cancel this order, min_out <= output");
        }

        let discriminator = [232, 219, 223, 41, 219, 236, 220, 190];

        let mut data = discriminator.to_vec();
        data.extend_from_slice(&solana_proof.proof_a);
        data.extend_from_slice(&solana_proof.proof_b);
        data.extend_from_slice(&solana_proof.proof_c);
        data.extend_from_slice(&public_inputs_arr[0]);
        data.extend_from_slice(&public_inputs_arr[1]);

        Ok(CancelAndAccountMetas {
            discriminator,
            cancel: DarklakeAmmCancelParams {
                proof_a: solana_proof.proof_a,
                proof_b: solana_proof.proof_b,
                proof_c: solana_proof.proof_c,
                public_signals: public_inputs_arr,
            },
            data,
            account_metas: 
                DarklakeAmmCancel {
                    caller: *settle_signer,
                    order_owner: *order_owner,
                    token_mint_x: self.pool.token_mint_x,
                    token_mint_y: self.pool.token_mint_y,
                    token_mint_wsol: native_mint::ID,
                    pool: self.key,
                    authority,
                    pool_token_reserve_x: self.pool.reserve_x,
                    pool_token_reserve_y: self.pool.reserve_y,
                    pool_wsol_reserve,
                    amm_config: self.pool.amm_config,
                    user_token_account_x,
                    user_token_account_y,
                    user_token_account_wsol,
                    caller_token_account_wsol,
                    order,
                    system_program: system_program::ID,
                    associated_token_program: spl_associated_token_account::ID,
                    token_mint_x_program: self.token_x_owner,
                    token_mint_y_program: self.token_y_owner,
                    token_program: spl_token::ID,
                }
                .into()
        })
    }
    

    fn get_slash_and_account_metas(&self, slash_params: &SlashParams) -> Result<SlashAndAccountMetas> {
        let SlashParams {
            settle_signer,
            order_owner,    
            deadline,
            current_slot,
        } = slash_params;

        if *current_slot <= *deadline {
            bail!("Order has NOT expired");
        }

        let authority = self.get_authority();

        let pool_wsol_reserve = self.get_pool_wsol_reserve();
        
        let user_token_account_x = self.get_user_token_account(*order_owner, self.pool.token_mint_x, self.token_x_owner);
        let user_token_account_y = self.get_user_token_account(*order_owner, self.pool.token_mint_y, self.token_y_owner);

        let caller_token_account_wsol = self.get_user_token_account(*settle_signer, native_mint::ID, spl_token::ID);
        let order = self.get_order(*order_owner);
    
        let discriminator = [204, 141, 18, 161, 8, 177, 92, 142];

        let data = discriminator.to_vec();

        Ok(SlashAndAccountMetas {
            discriminator,
            slash: DarklakeAmmSlashParams {},
            data,
            account_metas: 
                DarklakeAmmSlash {
                    caller: *settle_signer,
                    order_owner: *order_owner,
                    token_mint_x: self.pool.token_mint_x,
                    token_mint_y: self.pool.token_mint_y,
                    token_mint_wsol: native_mint::ID,
                    pool: self.key,
                    authority,
                    pool_token_reserve_x: self.pool.reserve_x,
                    pool_token_reserve_y: self.pool.reserve_y,
                    pool_wsol_reserve,
                    amm_config: self.pool.amm_config,
                    user_token_account_x,
                    user_token_account_y,
                    caller_token_account_wsol,
                    order,
                    system_program: system_program::ID,
                    associated_token_program: spl_associated_token_account::ID,
                    token_mint_x_program: self.token_x_owner,
                    token_mint_y_program: self.token_y_owner,
                    token_program: spl_token::ID,
                }
                .into()
        })
    }

    fn get_finalize_and_account_metas(&self, finalize_params: &FinalizeParams) -> Result<FinalizeAndAccountMetas> {
        let FinalizeParams {
            settle_signer,
            order_owner,
            unwrap_wsol,
            min_out,
            salt,
            output,
            commitment,
            deadline,
            current_slot,
        } = finalize_params;

        // check if settle or cancel or slash
        let is_settle = *min_out <= *output;
        let is_slash = *current_slot > *deadline;

        if is_slash {
            return Ok(FinalizeAndAccountMetas::Slash(self.get_slash_and_account_metas(&SlashParams {
                settle_signer: *settle_signer,
                order_owner: *order_owner,
                deadline: *deadline,
                current_slot: *current_slot,
            })?));
        } else if is_settle {
            return Ok(FinalizeAndAccountMetas::Settle(self.get_settle_and_account_metas(&SettleParams {
                settle_signer: *settle_signer,
                order_owner: *order_owner,
                unwrap_wsol: *unwrap_wsol,
                min_out: *min_out,
                salt: *salt,
                output: *output,
                commitment: *commitment,
                deadline: *deadline,
                current_slot: *current_slot,
            })?));
        } else {
            return Ok(FinalizeAndAccountMetas::Cancel(self.get_cancel_and_account_metas(&CancelParams {
                settle_signer: *settle_signer,
                order_owner: *order_owner,
                min_out: *min_out,
                salt: *salt,
                output: *output,
                commitment: *commitment,
                deadline: *deadline,
                current_slot: *current_slot,
            })?));
        }
    }

}

pub struct DarklakeAmmSwap {
    pub user: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,
    pub token_mint_wsol: Pubkey,
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub amm_config: Pubkey,
    pub user_token_account_x: Pubkey,
    pub user_token_account_y: Pubkey,
    pub user_token_account_wsol: Pubkey,
    pub pool_token_reserve_x: Pubkey,
    pub pool_token_reserve_y: Pubkey,
    pub pool_wsol_reserve: Pubkey,
    pub order: Pubkey,
    pub associated_token_program: Pubkey,
    pub system_program: Pubkey,
    pub token_mint_x_program: Pubkey,
    pub token_mint_y_program: Pubkey,
    pub token_program: Pubkey
}

impl From<DarklakeAmmSwap> for Vec<AccountMeta> {
    fn from(accounts: DarklakeAmmSwap) -> Self {
        vec![
            AccountMeta::new(accounts.user, true),
            AccountMeta::new_readonly(accounts.token_mint_x, false),
            AccountMeta::new_readonly(accounts.token_mint_y, false),
            AccountMeta::new_readonly(accounts.token_mint_wsol, false),
            AccountMeta::new(accounts.pool, false),
            AccountMeta::new_readonly(accounts.authority, false),
            AccountMeta::new_readonly(accounts.amm_config, false),
            AccountMeta::new(accounts.user_token_account_x, false),
            AccountMeta::new(accounts.user_token_account_y, false),
            AccountMeta::new(accounts.user_token_account_wsol, false),
            AccountMeta::new(accounts.pool_token_reserve_x, false),
            AccountMeta::new(accounts.pool_token_reserve_y, false),
            AccountMeta::new(accounts.pool_wsol_reserve, false),
            AccountMeta::new(accounts.order, false),
            AccountMeta::new_readonly(accounts.associated_token_program, false),
            AccountMeta::new_readonly(accounts.system_program, false),
            AccountMeta::new_readonly(accounts.token_mint_x_program, false),
            AccountMeta::new_readonly(accounts.token_mint_y_program, false),
            AccountMeta::new_readonly(accounts.token_program, false)
        ]
    }
}

pub struct DarklakeAmmSettle {
    pub caller: Pubkey,
    pub order_owner: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,
    pub token_mint_wsol: Pubkey,
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub pool_token_reserve_x: Pubkey,
    pub pool_token_reserve_y: Pubkey,
    pub pool_wsol_reserve: Pubkey,
    pub amm_config: Pubkey,
    pub user_token_account_x: Pubkey,
    pub user_token_account_y: Pubkey,
    pub user_token_account_wsol: Pubkey,
    pub caller_token_account_wsol: Pubkey,
    pub order: Pubkey,
    pub order_token_account_wsol: Pubkey,
    pub system_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub token_mint_x_program: Pubkey,
    pub token_mint_y_program: Pubkey,
    pub token_program: Pubkey
}

impl From<DarklakeAmmSettle> for Vec<AccountMeta> {
    fn from(accounts: DarklakeAmmSettle) -> Self {
        vec![
            AccountMeta::new(accounts.caller, true),
            AccountMeta::new(accounts.order_owner, false),
            AccountMeta::new_readonly(accounts.token_mint_x, false),
            AccountMeta::new_readonly(accounts.token_mint_y, false),
            AccountMeta::new_readonly(accounts.token_mint_wsol, false),
            AccountMeta::new(accounts.pool, false),
            AccountMeta::new_readonly(accounts.authority, false),
            AccountMeta::new(accounts.pool_token_reserve_x, false),
            AccountMeta::new(accounts.pool_token_reserve_y, false),
            AccountMeta::new(accounts.pool_wsol_reserve, false),
            AccountMeta::new_readonly(accounts.amm_config, false),
            AccountMeta::new(accounts.user_token_account_x, false),
            AccountMeta::new(accounts.user_token_account_y, false),
            AccountMeta::new(accounts.user_token_account_wsol, false),
            AccountMeta::new(accounts.caller_token_account_wsol, false),
            AccountMeta::new(accounts.order, false),
            AccountMeta::new(accounts.order_token_account_wsol, false),
            AccountMeta::new_readonly(accounts.system_program, false),
            AccountMeta::new_readonly(accounts.associated_token_program, false),
            AccountMeta::new_readonly(accounts.token_mint_x_program, false),
            AccountMeta::new_readonly(accounts.token_mint_y_program, false),
            AccountMeta::new_readonly(accounts.token_program, false)
        ]
    }
}

pub struct DarklakeAmmCancel {
    pub caller: Pubkey,
    pub order_owner: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,
    pub token_mint_wsol: Pubkey,
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub pool_token_reserve_x: Pubkey,
    pub pool_token_reserve_y: Pubkey,
    pub pool_wsol_reserve: Pubkey,
    pub amm_config: Pubkey,
    pub user_token_account_x: Pubkey,
    pub user_token_account_y: Pubkey,
    pub user_token_account_wsol: Pubkey,
    pub caller_token_account_wsol: Pubkey,
    pub order: Pubkey,
    pub system_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub token_mint_x_program: Pubkey,
    pub token_mint_y_program: Pubkey,
    pub token_program: Pubkey
}

impl From<DarklakeAmmCancel> for Vec<AccountMeta> {
    fn from(accounts: DarklakeAmmCancel) -> Self {
        vec![
            AccountMeta::new(accounts.caller, true),
            AccountMeta::new(accounts.order_owner, false),
            AccountMeta::new_readonly(accounts.token_mint_x, false),
            AccountMeta::new_readonly(accounts.token_mint_y, false),
            AccountMeta::new_readonly(accounts.token_mint_wsol, false),
            AccountMeta::new(accounts.pool, false),
            AccountMeta::new_readonly(accounts.authority, false),
            AccountMeta::new(accounts.pool_token_reserve_x, false),
            AccountMeta::new(accounts.pool_token_reserve_y, false),
            AccountMeta::new(accounts.pool_wsol_reserve, false),
            AccountMeta::new_readonly(accounts.amm_config, false),
            AccountMeta::new(accounts.user_token_account_x, false),
            AccountMeta::new(accounts.user_token_account_y, false),
            AccountMeta::new(accounts.user_token_account_wsol, false),
            AccountMeta::new(accounts.caller_token_account_wsol, false),
            AccountMeta::new(accounts.order, false),
            AccountMeta::new_readonly(accounts.system_program, false),
            AccountMeta::new_readonly(accounts.associated_token_program, false),
            AccountMeta::new_readonly(accounts.token_mint_x_program, false),
            AccountMeta::new_readonly(accounts.token_mint_y_program, false),
            AccountMeta::new_readonly(accounts.token_program, false)
        ]
    }
}


pub struct DarklakeAmmSlash {
    pub caller: Pubkey,
    pub order_owner: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,
    pub token_mint_wsol: Pubkey,
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub pool_token_reserve_x: Pubkey,
    pub pool_token_reserve_y: Pubkey,
    pub pool_wsol_reserve: Pubkey,
    pub amm_config: Pubkey,
    pub user_token_account_x: Pubkey,
    pub user_token_account_y: Pubkey,
    pub caller_token_account_wsol: Pubkey,
    pub order: Pubkey,
    pub system_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub token_mint_x_program: Pubkey,
    pub token_mint_y_program: Pubkey,
    pub token_program: Pubkey
}

impl From<DarklakeAmmSlash> for Vec<AccountMeta> {
    fn from(accounts: DarklakeAmmSlash) -> Self {
        vec![
            AccountMeta::new(accounts.caller, true),
            AccountMeta::new(accounts.order_owner, false),
            AccountMeta::new_readonly(accounts.token_mint_x, false),
            AccountMeta::new_readonly(accounts.token_mint_y, false),
            AccountMeta::new_readonly(accounts.token_mint_wsol, false),
            AccountMeta::new(accounts.pool, false),
            AccountMeta::new_readonly(accounts.authority, false),
            AccountMeta::new(accounts.pool_token_reserve_x, false),
            AccountMeta::new(accounts.pool_token_reserve_y, false),
            AccountMeta::new(accounts.pool_wsol_reserve, false),
            AccountMeta::new_readonly(accounts.amm_config, false),
            AccountMeta::new(accounts.user_token_account_x, false),
            AccountMeta::new(accounts.user_token_account_y, false),
            AccountMeta::new(accounts.caller_token_account_wsol, false),
            AccountMeta::new(accounts.order, false),
            AccountMeta::new_readonly(accounts.system_program, false),
            AccountMeta::new_readonly(accounts.associated_token_program, false),
            AccountMeta::new_readonly(accounts.token_mint_x_program, false),
            AccountMeta::new_readonly(accounts.token_mint_y_program, false),
            AccountMeta::new_readonly(accounts.token_program, false)
        ]
    }
}

impl DarklakeAmm {
    /// Parse token account balance reliably for both SPL and Token2022 tokens
    fn parse_token_account_balance(account_data: &[u8], account_owner: &Pubkey, token_account_pubkey: &Pubkey) -> Result<u64> {
        // Check which token program owns the account and parse accordingly
        match account_owner {
            owner if *owner == spl_token::ID => {
                // SPL Token account - use proper unpacking
                let token_account = SplTokenAccount::unpack(&account_data)?;
                Ok(token_account.amount)
            }
            owner if *owner == spl_token_2022::ID => {
                // Token2022 account - use StateWithExtensions for proper parsing
                let token_account = StateWithExtensions::<spl_token_2022::state::Account>::unpack(account_data)?;
                Ok(token_account.base.amount)
            }
            _ => {
                bail!("Unknown token program: {} for account {}", account_owner, token_account_pubkey);
            }
        }
    }

    fn get_transfer_fee_config(&self, mint_account_data: &[u8], mint_owner: &Pubkey) -> Result<Option<TransferFeeConfig>, ()> {
        // Only Token2022 tokens can have transfer fee configs
        if *mint_owner != spl_token_2022::ID {
            return Ok(None);
        }
        
        // Try to parse as Token2022 mint, but handle errors gracefully
        match StateWithExtensions::<spl_token_2022::state::Mint>::unpack(mint_account_data) {
            Ok(mint) => {
                // Successfully parsed as Token2022 mint, try to get transfer fee config
                match mint.get_extension::<TransferFeeConfig>() {
                    Ok(transfer_fee_config) => Ok(Some(transfer_fee_config.clone())),
                    Err(_) => Ok(None), // Extension not found or error getting it
                }
            }
            Err(_) => {
                // Not a valid Token2022 mint or some other error occurred
                // Return None instead of an error, indicating no transfer fee config
                Ok(None)
            }
        }
    }

    fn quote_internal(
        &self,
        swap_params: &QuoteParams,
        amm_config: &AmmConfig,
        reserve_x_balance: u64,
        reserve_y_balance: u64
    ) -> Result<Quote> {

        let is_swap_x_to_y = swap_params.input_mint == self.pool.token_mint_x;

        let amount_in = swap_params.amount;

           // exclude protocol fees / locked pool reserves / user pending orders
           let (total_token_x_amount, total_token_y_amount) = (
            reserve_x_balance
                .checked_sub(self.pool.protocol_fee_x)
                .unwrap()
                .checked_sub(self.pool.user_locked_x)
                .unwrap(),
            reserve_y_balance
                .checked_sub(self.pool.protocol_fee_y)
                .unwrap()
                .checked_sub(self.pool.user_locked_y)
                .unwrap(),
        );

        let (available_token_x_amount, available_token_y_amount) = (
            total_token_x_amount
                .checked_sub(self.pool.locked_x)
                .unwrap(),
            total_token_y_amount
                .checked_sub(self.pool.locked_y)
                .unwrap(),
        );

        // the amount we receive excluding any outside transfer fees
        let exchange_in;
        // Calculate the output amount using the constant product formula
        let result_amounts: SwapResultWithFromToLock = if is_swap_x_to_y {
            // Swap X to Y

            let input_transfer_fee =
                get_transfer_fee(&self.token_x_transfer_fee_config, amount_in)?;

            // Take transfer fees into account for actual amount transferred in
            exchange_in = amount_in.saturating_sub(input_transfer_fee);

            if exchange_in == 0 {
                bail!("Input amount too small");
            }

            let result_amounts = swap(
                exchange_in as u128,
                available_token_x_amount as u128,
                available_token_y_amount as u128,
                self.amm_config.trade_fee_rate,
                self.amm_config.protocol_fee_rate,
            )
            .ok_or(anyhow::anyhow!("Math overflow"))?;

            let rebalance_result = rebalance_pool_ratio(
                result_amounts.to_amount,
                available_token_x_amount,
                available_token_y_amount,
                total_token_x_amount,
                total_token_y_amount,
                self.amm_config.ratio_change_tolerance_rate,
            )
            .ok_or(anyhow::anyhow!("Math overflow"))?;

            if rebalance_result.is_rate_tolerance_exceeded {
                bail!("Trade too big");
            }

            // can't reserve to 0 or negative
            if rebalance_result.from_to_lock >= available_token_x_amount {
                bail!("Insufficient pool token X balance");
            }

            SwapResultWithFromToLock {
                from_amount: result_amounts.from_amount, // applied trade fee + transfer fee
                to_amount: result_amounts.to_amount,     // nothing applied
                from_to_lock: rebalance_result.from_to_lock,
                trade_fee: result_amounts.trade_fee,
                protocol_fee: result_amounts.protocol_fee,
            }
        } else {
            let input_transfer_fee =
                get_transfer_fee(&self.token_y_transfer_fee_config, amount_in)?;
            // Take transfer fees into account for actual amount transferred in
            exchange_in = amount_in.saturating_sub(input_transfer_fee);
            if exchange_in == 0 {
                bail!("Input amount too small");
            }
            // Swap Y to X
            let result_amounts = swap(
                exchange_in as u128,
                available_token_y_amount as u128,
                available_token_x_amount as u128,
                self.amm_config.trade_fee_rate,
                self.amm_config.protocol_fee_rate,
            )
            .ok_or(anyhow::anyhow!("Math overflow"))?;

            let rebalance_result = rebalance_pool_ratio(
                result_amounts.to_amount,
                available_token_y_amount,
                available_token_x_amount,
                total_token_y_amount,
                total_token_x_amount,
                self.amm_config.ratio_change_tolerance_rate,
            )
            .ok_or(anyhow::anyhow!("Math overflow"))?;

            if rebalance_result.is_rate_tolerance_exceeded {
                bail!("Trade too big");
            }

            // can't reserve to 0 or negative
            if rebalance_result.from_to_lock > available_token_y_amount {
                bail!("Insufficient pool token Y balance");
            }

            SwapResultWithFromToLock {
                from_amount: result_amounts.from_amount, // applied trade fee + transfer fee
                to_amount: result_amounts.to_amount,     // nothing applied
                from_to_lock: rebalance_result.from_to_lock,
                trade_fee: result_amounts.trade_fee,
                protocol_fee: result_amounts.protocol_fee,
            }
        };

        let output_mint = if is_swap_x_to_y {
            self.pool.token_mint_y
        } else {
            self.pool.token_mint_x
        };

        let output_transfer_fee_config = if output_mint == self.pool.token_mint_x {
            self.token_x_transfer_fee_config
        } else {
            self.token_y_transfer_fee_config
        };
        let output_transfer_fee = get_transfer_fee(&output_transfer_fee_config, result_amounts.to_amount as u64)?;

        // Take transfer fees into account for actual amount transferred in
        let actual_output_amount = (result_amounts.to_amount as u64)
            .checked_sub(output_transfer_fee)
            .unwrap();

        if actual_output_amount == 0 {
            bail!("Output amount is zero");
        }

        let fee_pct = Decimal::new(amm_config.trade_fee_rate as i64, 4);
        Ok(Quote { in_amount: amount_in, out_amount: actual_output_amount, fee_amount: output_transfer_fee, fee_mint: swap_params.input_mint, fee_pct })
    }

    fn get_authority(&self) -> Pubkey {
        Pubkey::find_program_address(&[b"authority"], &self.program_id()).0
    }

    fn get_user_token_account(&self, user: Pubkey, token_mint: Pubkey, token_program: Pubkey) -> Pubkey {
        Pubkey::find_program_address(&[user.as_ref(), token_program.as_ref(), token_mint.as_ref()], &spl_associated_token_account::ID).0
    }

    fn get_pool_wsol_reserve(&self) -> Pubkey {
        Pubkey::find_program_address(&[b"pool_wsol_reserve", self.key.as_ref()], &self.program_id()).0
    }

    fn get_order(&self, user: Pubkey) -> Pubkey {
        Pubkey::find_program_address(&[b"order", self.key.as_ref(), user.as_ref()], &self.program_id()).0
    }

    fn get_order_token_account_wsol(&self, user: Pubkey) -> Pubkey {
        Pubkey::find_program_address(&[b"order_wsol", self.key.as_ref(), user.as_ref()], &self.program_id()).0
    }
}
