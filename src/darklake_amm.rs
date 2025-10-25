use anchor_lang::prelude::*;
use dex_math::quote;
use solana_sdk::sysvar::SysvarId;

use crate::account_metas::DarklakeAmmInitializePool;
use crate::constants::{
    AMM_CONFIG, AUTHORITY, DARKLAKE_PROGRAM_ID, DEVNET_CREATE_POOL_FEE_VAULT, LIQUIDITY_SEED,
    MAINNET_CREATE_POOL_FEE_VAULT, METADATA_PROGRAM_ID, METADATA_SEED, ORDER_SEED, ORDER_WSOL_SEED,
    POOL_RESERVE_SEED, POOL_SEED, POOL_WSOL_RESERVE_SEED,
};
use crate::proof::proof_generator::to_32_byte_buffer;
use crate::proof::utils::{
    bytes_to_bigint, compute_poseidon_hash_with_salt, u64_array_to_u8_array_le,
};
use crate::utils::get_transfer_fee;
use crate::{
    account_metas::{
        DarklakeAmmAddLiquidity, DarklakeAmmCancel, DarklakeAmmRemoveLiquidity, DarklakeAmmSettle,
        DarklakeAmmSlash, DarklakeAmmSwap,
    },
    amm::*,
};

use anchor_lang::{AnchorDeserialize, AnchorSerialize, system_program};
use anyhow::{Context, Result, bail};
use rust_decimal::Decimal;
use solana_sdk::{program_pack::Pack, pubkey::Pubkey};
use spl_token::{native_mint, state::Account as SplTokenAccount};
use spl_token_2022::extension::{
    BaseStateWithExtensions, StateWithExtensions, transfer_fee::TransferFeeConfig,
};

#[derive(Clone)]
pub(crate) struct DarklakeAmm {
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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct AmmConfig {
    pub trade_fee_rate: u64,
    pub create_pool_fee: u64,
    pub protocol_fee_rate: u64,

    pub wsol_trade_deposit: u64,

    pub deadline_slot_duration: u64,

    pub ratio_change_tolerance_rate: u64,

    pub bump: u8,
    pub halted: bool,

    pub padding: [u64; 16],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Order {
    pub trader: Pubkey,
    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,

    pub actual_in: u64,
    pub exchange_in: u64,
    pub actual_out: u64,
    pub from_to_lock: u64,
    pub d_in: u64,
    pub d_out: u64,
    pub deadline: u64,
    pub protocol_fee: u64,
    pub wsol_deposit: u64,

    pub c_min: [u8; 32],

    pub is_x_to_y: bool,
    pub bump: u8,

    pub lp_fee: u64,

    pub padding: [u64; 3],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct Pool {
    pub creator: Pubkey,
    pub amm_config: Pubkey,

    pub token_mint_x: Pubkey,
    pub token_mint_y: Pubkey,

    pub reserve_x: Pubkey,
    pub reserve_y: Pubkey,

    pub token_lp_supply: u64,
    pub protocol_fee_x: u64,
    pub protocol_fee_y: u64,

    pub locked_x: u64,
    pub locked_y: u64,

    pub user_locked_x: u64,
    pub user_locked_y: u64,

    pub bump: u8,

    pub lp_fee_x: u64,
    pub lp_fee_y: u64,

    pub padding: [u64; 2],
}

impl Amm for DarklakeAmm {
    fn load_pool(pool: &KeyedAccount) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(DarklakeAmm {
            key: pool.key,
            pool: Pool::deserialize(&mut &pool.account.data[8..])?,
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
            self.pool.token_mint_x,
            self.pool.token_mint_y,
            self.pool.reserve_x,
            self.pool.reserve_y,
            self.pool.amm_config,
        ]
    }

    fn update(&mut self, account_map: &AccountMap) -> Result<()> {
        let account = account_map
            .get(&self.key)
            .context("Darklake pool account not found")?;

        self.pool = Pool::deserialize(&mut &account.data[8..])?;

        let amm_config_data = try_get_account_data(account_map, &self.pool.amm_config)?;
        let amm_config = AmmConfig::deserialize(&mut &amm_config_data[8..])?;

        self.amm_config = amm_config;

        let (token_x_data, token_x_owner) =
            try_get_account_data_and_owner(account_map, &self.pool.reserve_x)?;
        let (token_y_data, token_y_owner) =
            try_get_account_data_and_owner(account_map, &self.pool.reserve_y)?;

        self.reserve_x_balance =
            Self::parse_token_account_balance(&token_x_data, &token_x_owner, &self.pool.reserve_x)?;
        self.reserve_y_balance =
            Self::parse_token_account_balance(&token_y_data, &token_y_owner, &self.pool.reserve_y)?;

        self.token_x_transfer_fee_config = self
            .get_transfer_fee_config(&token_x_data, token_x_owner)
            .unwrap_or(None);

        self.token_y_transfer_fee_config = self
            .get_transfer_fee_config(&token_y_data, token_y_owner)
            .unwrap_or(None);

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

        let is_swap_x_to_y = quote_params.input_mint == self.pool.token_mint_x;

        let amm_config = dex_math::AmmConfig {
            trade_fee_rate: self.amm_config.trade_fee_rate,
            protocol_fee_rate: self.amm_config.protocol_fee_rate,
            ratio_change_tolerance_rate: self.amm_config.ratio_change_tolerance_rate,
        };

        let input_transfer_fee = if is_swap_x_to_y {
            get_transfer_fee(
                self.token_x_transfer_fee_config,
                quote_params.amount,
                quote_params.epoch,
            )?
        } else {
            get_transfer_fee(
                self.token_y_transfer_fee_config,
                quote_params.amount,
                quote_params.epoch,
            )?
        };

        let exchange_in = quote_params.amount.checked_sub(input_transfer_fee).unwrap();

        let result = quote(
            exchange_in,
            is_swap_x_to_y,
            &amm_config,
            self.pool.protocol_fee_x,
            self.pool.protocol_fee_y,
            self.pool.user_locked_x,
            self.pool.user_locked_y,
            self.pool.locked_x,
            self.pool.locked_y,
            self.reserve_x_balance,
            self.reserve_y_balance,
            self.pool.lp_fee_x,
            self.pool.lp_fee_y,
        )?;

        let output_transfer_fee = if is_swap_x_to_y {
            get_transfer_fee(
                self.token_y_transfer_fee_config,
                result.to_amount,
                quote_params.epoch,
            )?
        } else {
            get_transfer_fee(
                self.token_x_transfer_fee_config,
                result.to_amount,
                quote_params.epoch,
            )?
        };

        let actual_output_amount = result.to_amount.checked_sub(output_transfer_fee).unwrap();

        if actual_output_amount == 0 {
            bail!("Output is zero");
        }

        Ok(Quote {
            in_amount: result.from_amount,
            out_amount: actual_output_amount,
            fee_amount: result.trade_fee,
            fee_mint: if is_swap_x_to_y {
                self.pool.token_mint_x
            } else {
                self.pool.token_mint_y
            },
            fee_pct: Decimal::from(self.amm_config.trade_fee_rate),
        })
    }

    fn get_swap_and_account_metas(&self, swap_params: &SwapParams) -> Result<SwapAndAccountMetas> {
        let SwapParams {
            token_transfer_authority,
            salt,
            min_out,
            label,
            ..
        } = swap_params;

        let authority = AUTHORITY.key();
        let is_swap_x_to_y = swap_params.source_mint == self.pool.token_mint_x;

        let user_token_account_wsol = DarklakeAmm::get_user_token_account(
            *token_transfer_authority,
            native_mint::ID,
            spl_token::ID,
        );
        let user_token_account_x = DarklakeAmm::get_user_token_account(
            *token_transfer_authority,
            self.pool.token_mint_x,
            self.token_x_owner,
        );
        let user_token_account_y = DarklakeAmm::get_user_token_account(
            *token_transfer_authority,
            self.pool.token_mint_y,
            self.token_y_owner,
        );

        let pool_wsol_reserve = DarklakeAmm::get_pool_wsol_reserve(self.key);
        let order = self.get_order(token_transfer_authority);

        let commitment = to_32_byte_buffer(&bytes_to_bigint(&u64_array_to_u8_array_le(
            &compute_poseidon_hash_with_salt(*min_out, *salt),
        )));
        let discriminator = [248, 198, 158, 145, 225, 117, 135, 200];

        let mut data = discriminator.to_vec();

        data.extend_from_slice(&swap_params.amount_in.to_le_bytes());
        data.extend_from_slice(&[is_swap_x_to_y as u8]);
        data.extend_from_slice(&commitment);
        let serialized_label = label.try_to_vec()?;
        data.extend_from_slice(&serialized_label);

        Ok(SwapAndAccountMetas {
            discriminator,
            swap: DarklakeAmmSwapParams {
                amount_in: swap_params.amount_in,
                is_swap_x_to_y,
                c_min: commitment,
                label: *label,
            },
            data,
            account_metas: DarklakeAmmSwap {
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
            .into(),
        })
    }

    fn clone_amm(&self) -> Box<dyn Amm + Send + Sync> {
        Box::new(self.clone())
    }

    fn is_active(&self) -> bool {
        !self.amm_config.halted
    }

    fn get_order_pubkey(&self, user: &Pubkey) -> Result<Pubkey> {
        if self.key == Pubkey::default() {
            bail!("Darklake pool is not initialized");
        }
        Ok(self.get_order(user))
    }

    fn parse_order_data(&self, order_data: &[u8]) -> Result<Order> {
        let order = Order::deserialize(&mut &order_data[8..])?;
        Ok(order)
    }

    fn is_order_expired(&self, order_data: &[u8], current_slot: u64) -> Result<bool> {
        let order = Order::deserialize(&mut &order_data[8..])?;
        Ok(order.deadline < current_slot)
    }

    fn get_settle_and_account_metas(
        &self,
        settle_params: &SettleParams,
        proof_params: &ProofParams,
    ) -> Result<SettleAndAccountMetas> {
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
            ref_code,
            label,
        } = settle_params;

        if *current_slot > *deadline {
            bail!("Order has expired");
        }

        let authority = AUTHORITY.key();

        let pool_wsol_reserve = DarklakeAmm::get_pool_wsol_reserve(self.key);

        let user_token_account_wsol =
            DarklakeAmm::get_user_token_account(*order_owner, native_mint::ID, spl_token::ID);
        let user_token_account_x = DarklakeAmm::get_user_token_account(
            *order_owner,
            self.pool.token_mint_x,
            self.token_x_owner,
        );
        let user_token_account_y = DarklakeAmm::get_user_token_account(
            *order_owner,
            self.pool.token_mint_y,
            self.token_y_owner,
        );

        let caller_token_account_wsol =
            DarklakeAmm::get_user_token_account(*settle_signer, native_mint::ID, spl_token::ID);
        let order = self.get_order(order_owner);
        let order_token_account_wsol = self.get_order_token_account_wsol(*order_owner);

        let is_settle = *min_out <= *output;

        if !is_settle {
            bail!("Cant settle this order, min_out > output");
        }
        let discriminator = [175, 42, 185, 87, 144, 131, 102, 212];

        let mut data = discriminator.to_vec();
        data.extend_from_slice(&proof_params.generated_proof.proof_a);
        data.extend_from_slice(&proof_params.generated_proof.proof_b);
        data.extend_from_slice(&proof_params.generated_proof.proof_c);
        data.extend_from_slice(&proof_params.public_inputs[0]);
        data.extend_from_slice(&proof_params.public_inputs[1]);
        data.extend_from_slice(&[*unwrap_wsol as u8]);
        let serialized_ref_code = ref_code.try_to_vec()?;
        data.extend_from_slice(&serialized_ref_code);
        let serialized_label = label.try_to_vec()?;
        data.extend_from_slice(&serialized_label);

        Ok(SettleAndAccountMetas {
            discriminator,
            settle: DarklakeAmmSettleParams {
                proof_a: proof_params.generated_proof.proof_a,
                proof_b: proof_params.generated_proof.proof_b,
                proof_c: proof_params.generated_proof.proof_c,
                public_signals: proof_params.public_inputs,
                unwrap_wsol: *unwrap_wsol,
                ref_code: *ref_code,
                label: *label,
            },
            data,
            account_metas: DarklakeAmmSettle {
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
            .into(),
        })
    }

    fn get_cancel_and_account_metas(
        &self,
        cancel_params: &CancelParams,
        proof_params: &ProofParams,
    ) -> Result<CancelAndAccountMetas> {
        let CancelParams {
            settle_signer,
            order_owner,
            min_out,
            salt,
            output,
            commitment,
            deadline,
            current_slot,
            label,
        } = cancel_params;

        if *current_slot > *deadline {
            bail!("Order has expired");
        }

        let authority = AUTHORITY.key();

        let pool_wsol_reserve = DarklakeAmm::get_pool_wsol_reserve(self.key);

        let user_token_account_wsol =
            DarklakeAmm::get_user_token_account(*order_owner, native_mint::ID, spl_token::ID);
        let user_token_account_x = DarklakeAmm::get_user_token_account(
            *order_owner,
            self.pool.token_mint_x,
            self.token_x_owner,
        );
        let user_token_account_y = DarklakeAmm::get_user_token_account(
            *order_owner,
            self.pool.token_mint_y,
            self.token_y_owner,
        );

        let caller_token_account_wsol =
            DarklakeAmm::get_user_token_account(*settle_signer, native_mint::ID, spl_token::ID);
        let order = self.get_order(order_owner);

        let is_cancel = *min_out > *output;

        if !is_cancel {
            bail!("Cant cancel this order, min_out <= output");
        }

        let discriminator = [232, 219, 223, 41, 219, 236, 220, 190];

        let mut data = discriminator.to_vec();
        data.extend_from_slice(&proof_params.generated_proof.proof_a);
        data.extend_from_slice(&proof_params.generated_proof.proof_b);
        data.extend_from_slice(&proof_params.generated_proof.proof_c);
        data.extend_from_slice(&proof_params.public_inputs[0]);
        data.extend_from_slice(&proof_params.public_inputs[1]);
        let serialized_label = label.try_to_vec()?;
        data.extend_from_slice(&serialized_label);

        Ok(CancelAndAccountMetas {
            discriminator,
            cancel: DarklakeAmmCancelParams {
                proof_a: proof_params.generated_proof.proof_a,
                proof_b: proof_params.generated_proof.proof_b,
                proof_c: proof_params.generated_proof.proof_c,
                public_signals: proof_params.public_inputs,
                label: *label,
            },
            data,
            account_metas: DarklakeAmmCancel {
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
            .into(),
        })
    }

    fn get_slash_and_account_metas(
        &self,
        slash_params: &SlashParams,
    ) -> Result<SlashAndAccountMetas> {
        let SlashParams {
            settle_signer,
            order_owner,
            deadline,
            current_slot,
            label,
        } = slash_params;

        if *current_slot <= *deadline {
            bail!("Order has NOT expired");
        }

        let authority = AUTHORITY.key();

        let pool_wsol_reserve = DarklakeAmm::get_pool_wsol_reserve(self.key);

        let user_token_account_x = DarklakeAmm::get_user_token_account(
            *order_owner,
            self.pool.token_mint_x,
            self.token_x_owner,
        );
        let user_token_account_y = DarklakeAmm::get_user_token_account(
            *order_owner,
            self.pool.token_mint_y,
            self.token_y_owner,
        );

        let caller_token_account_wsol =
            DarklakeAmm::get_user_token_account(*settle_signer, native_mint::ID, spl_token::ID);
        let order = self.get_order(order_owner);

        let discriminator = [204, 141, 18, 161, 8, 177, 92, 142];

        let mut data = discriminator.to_vec();
        let serialized_label = label.try_to_vec()?;
        data.extend_from_slice(&serialized_label);

        Ok(SlashAndAccountMetas {
            discriminator,
            slash: DarklakeAmmSlashParams { label: *label },
            data,
            account_metas: DarklakeAmmSlash {
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
            .into(),
        })
    }

    fn get_add_liquidity_and_account_metas(
        &self,
        add_liquidity_params: &AddLiquidityParams,
    ) -> Result<AddLiquidityAndAccountMetas> {
        let AddLiquidityParams {
            amount_lp,
            max_amount_x,
            max_amount_y,
            user,
            ref_code,
            label,
        } = add_liquidity_params;

        let authority = AUTHORITY.key();

        let user_token_account_x =
            DarklakeAmm::get_user_token_account(*user, self.pool.token_mint_x, self.token_x_owner);
        let user_token_account_y =
            DarklakeAmm::get_user_token_account(*user, self.pool.token_mint_y, self.token_y_owner);

        let token_mint_lp = DarklakeAmm::get_token_mint_lp(self.key);

        let user_token_account_lp =
            DarklakeAmm::get_user_token_account(*user, token_mint_lp, spl_token::ID);

        let discriminator = [181, 157, 89, 67, 143, 182, 52, 72];

        let mut data = discriminator.to_vec();
        data.extend_from_slice(&amount_lp.to_le_bytes());
        data.extend_from_slice(&max_amount_x.to_le_bytes());
        data.extend_from_slice(&max_amount_y.to_le_bytes());
        let serialized_ref_code = ref_code.try_to_vec()?;
        data.extend_from_slice(&serialized_ref_code);
        let serialized_label = label.try_to_vec()?;
        data.extend_from_slice(&serialized_label);

        Ok(AddLiquidityAndAccountMetas {
            discriminator,
            add_liquidity: DarklakeAmmAddLiquidityParams {
                amount_lp: *amount_lp,
                max_amount_x: *max_amount_x,
                max_amount_y: *max_amount_y,
                ref_code: *ref_code,
                label: *label,
            },
            data,
            account_metas: DarklakeAmmAddLiquidity {
                user: *user,
                token_mint_x: self.pool.token_mint_x,
                token_mint_y: self.pool.token_mint_y,
                token_mint_lp,
                pool: self.key,
                authority,
                pool_token_reserve_x: self.pool.reserve_x,
                pool_token_reserve_y: self.pool.reserve_y,
                amm_config: self.pool.amm_config,
                user_token_account_x,
                user_token_account_y,
                user_token_account_lp,
                system_program: system_program::ID,
                associated_token_program: spl_associated_token_account::ID,
                token_mint_x_program: self.token_x_owner,
                token_mint_y_program: self.token_y_owner,
                token_program: spl_token::ID,
            }
            .into(),
        })
    }

    fn get_remove_liquidity_and_account_metas(
        &self,
        remove_liquidity_params: &RemoveLiquidityParams,
    ) -> Result<RemoveLiquidityAndAccountMetas> {
        let RemoveLiquidityParams {
            amount_lp,
            min_amount_x,
            min_amount_y,
            user,
            label,
        } = remove_liquidity_params;

        let authority = AUTHORITY.key();

        let user_token_account_x =
            DarklakeAmm::get_user_token_account(*user, self.pool.token_mint_x, self.token_x_owner);
        let user_token_account_y =
            DarklakeAmm::get_user_token_account(*user, self.pool.token_mint_y, self.token_y_owner);

        let token_mint_lp = DarklakeAmm::get_token_mint_lp(self.key);

        let user_token_account_lp =
            DarklakeAmm::get_user_token_account(*user, token_mint_lp, spl_token::ID);

        let discriminator = [80, 85, 209, 72, 24, 206, 177, 108];

        let mut data = discriminator.to_vec();
        data.extend_from_slice(&amount_lp.to_le_bytes());
        data.extend_from_slice(&min_amount_x.to_le_bytes());
        data.extend_from_slice(&min_amount_y.to_le_bytes());
        let serialized_label = label.try_to_vec()?;
        data.extend_from_slice(&serialized_label);

        Ok(RemoveLiquidityAndAccountMetas {
            discriminator,
            remove_liquidity: DarklakeAmmRemoveLiquidityParams {
                amount_lp: *amount_lp,
                min_amount_x: *min_amount_x,
                min_amount_y: *min_amount_y,
                label: *label,
            },
            data,
            account_metas: DarklakeAmmRemoveLiquidity {
                user: *user,
                token_mint_x: self.pool.token_mint_x,
                token_mint_y: self.pool.token_mint_y,
                token_mint_lp,
                pool: self.key,
                authority,
                pool_token_reserve_x: self.pool.reserve_x,
                pool_token_reserve_y: self.pool.reserve_y,
                amm_config: self.pool.amm_config,
                user_token_account_x,
                user_token_account_y,
                user_token_account_lp,
                system_program: system_program::ID,
                associated_token_program: spl_associated_token_account::ID,
                token_mint_x_program: self.token_x_owner,
                token_mint_y_program: self.token_y_owner,
                token_program: spl_token::ID,
            }
            .into(),
        })
    }

    fn get_initialize_pool_and_account_metas(
        &self,
        initialize_pool_params: &InitializePoolParams,
        is_devnet: bool,
    ) -> Result<InitializePoolAndAccountMetas> {
        let InitializePoolParams {
            user,
            token_x,
            token_x_program,
            token_y,
            token_y_program,
            amount_x,
            amount_y,
            label,
        } = initialize_pool_params;

        let authority = AUTHORITY.key();
        let pool_address = DarklakeAmm::get_pool_address(token_x, token_y);

        let user_token_account_x =
            DarklakeAmm::get_user_token_account(*user, *token_x, *token_x_program);
        let user_token_account_y =
            DarklakeAmm::get_user_token_account(*user, *token_y, *token_y_program);

        let token_mint_lp = DarklakeAmm::get_token_mint_lp(pool_address);

        let metadata_account = DarklakeAmm::get_token_metadata(token_mint_lp);
        let metadata_account_x = DarklakeAmm::get_token_metadata(*token_x);
        let metadata_account_y = DarklakeAmm::get_token_metadata(*token_y);

        let user_token_account_lp =
            DarklakeAmm::get_user_token_account(*user, token_mint_lp, spl_token::ID);

        let discriminator = [95, 180, 10, 172, 84, 174, 232, 40];

        let mut data = discriminator.to_vec();
        data.extend_from_slice(&amount_x.to_le_bytes());
        data.extend_from_slice(&amount_y.to_le_bytes());
        let serialized_label = label.try_to_vec()?;
        data.extend_from_slice(&serialized_label);

        Ok(InitializePoolAndAccountMetas {
            discriminator,
            initialize_pool: DarklakeAmmInitializePoolParams {
                amount_x: *amount_x,
                amount_y: *amount_y,
                label: *label,
            },
            data,
            account_metas: DarklakeAmmInitializePool {
                user: *user,
                pool: pool_address,
                authority,
                amm_config: *AMM_CONFIG,
                token_mint_x: *token_x,
                token_mint_y: *token_y,
                token_mint_wsol: native_mint::ID,
                token_mint_lp,
                metadata_account,
                metadata_account_x,
                metadata_account_y,
                user_token_account_x,
                user_token_account_y,
                user_token_account_lp,
                pool_token_reserve_x: DarklakeAmm::get_pool_reserve(pool_address, *token_x),
                pool_token_reserve_y: DarklakeAmm::get_pool_reserve(pool_address, *token_y),
                pool_wsol_reserve: DarklakeAmm::get_pool_wsol_reserve(pool_address),
                create_pool_fee_vault: if is_devnet {
                    DEVNET_CREATE_POOL_FEE_VAULT
                } else {
                    MAINNET_CREATE_POOL_FEE_VAULT
                },
                mpl_program: METADATA_PROGRAM_ID,
                system_program: system_program::ID,
                rent: Rent::id(),
                associated_token_program: spl_associated_token_account::ID,
                token_mint_x_program: *token_x_program,
                token_mint_y_program: *token_y_program,
                token_program: spl_token::ID,
            }
            .into(),
        })
    }
}

impl DarklakeAmm {
    fn parse_token_account_balance(
        account_data: &[u8],
        account_owner: &Pubkey,
        token_account_pubkey: &Pubkey,
    ) -> Result<u64> {
        match account_owner {
            owner if *owner == spl_token::ID => {
                let token_account = SplTokenAccount::unpack(&account_data)?;
                Ok(token_account.amount)
            }
            owner if *owner == spl_token_2022::ID => {
                let token_account =
                    StateWithExtensions::<spl_token_2022::state::Account>::unpack(account_data)?;
                Ok(token_account.base.amount)
            }
            _ => {
                bail!(
                    "Unknown token program: {} for account {}",
                    account_owner,
                    token_account_pubkey
                );
            }
        }
    }

    fn get_transfer_fee_config(
        &self,
        mint_account_data: &[u8],
        mint_owner: &Pubkey,
    ) -> Result<Option<TransferFeeConfig>, ()> {
        if *mint_owner != spl_token_2022::ID {
            return Ok(None);
        }

        match StateWithExtensions::<spl_token_2022::state::Mint>::unpack(mint_account_data) {
            Ok(mint) => {
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

    pub fn get_user_token_account(
        user: Pubkey,
        token_mint: Pubkey,
        token_program: Pubkey,
    ) -> Pubkey {
        Pubkey::find_program_address(
            &[user.as_ref(), token_program.as_ref(), token_mint.as_ref()],
            &spl_associated_token_account::ID,
        )
        .0
    }

    pub fn get_pool_wsol_reserve(pool: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[POOL_WSOL_RESERVE_SEED, pool.as_ref()],
            &DARKLAKE_PROGRAM_ID,
        )
        .0
    }

    fn get_order(&self, user: &Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[ORDER_SEED, self.key.as_ref(), user.as_ref()],
            &self.program_id(),
        )
        .0
    }

    pub fn get_token_mint_lp(pool: Pubkey) -> Pubkey {
        Pubkey::find_program_address(&[LIQUIDITY_SEED, pool.as_ref()], &DARKLAKE_PROGRAM_ID).0
    }

    fn get_order_token_account_wsol(&self, user: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[ORDER_WSOL_SEED, self.key.as_ref(), user.as_ref()],
            &self.program_id(),
        )
        .0
    }

    pub fn get_pool_address(token_mint_x: &Pubkey, token_mint_y: &Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[
                POOL_SEED,
                &AMM_CONFIG.as_ref(),
                token_mint_x.as_ref(),
                token_mint_y.as_ref(),
            ],
            &DARKLAKE_PROGRAM_ID,
        )
        .0
    }

    pub fn get_token_metadata(token_mint: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[
                METADATA_SEED,
                &METADATA_PROGRAM_ID.as_ref(),
                token_mint.as_ref(),
            ],
            &METADATA_PROGRAM_ID,
        )
        .0
    }

    pub fn get_pool_reserve(pool: Pubkey, token_mint: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[POOL_RESERVE_SEED, pool.as_ref(), token_mint.as_ref()],
            &DARKLAKE_PROGRAM_ID,
        )
        .0
    }

    pub fn get_token_owners(&self) -> (Pubkey, Pubkey) {
        (self.token_x_owner, self.token_y_owner)
    }
}
