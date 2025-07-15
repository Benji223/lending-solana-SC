use std::f32::consts::E;

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{ self, Mint, TokenAccount, TokenInterface, TransferChecked };

use crate::state::*;

use crate::error::ErrorCode;



#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut, 
        seeds = [mint.key().as_ref()],
        bump,
    )]  
    pub bank: Account<'info, Bank>,

    #[account(
        mut, 
        seeds = [b"treasury", mint.key().as_ref()],
        bump, 
    )]
    pub bank_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut, 
        seeds = [signer.key().as_ref()],
        bump,
    )]
    pub user_account: Account<'info, User>,

    #[account( 
        init_if_needed,
        payer = signer,
        associated_token::mint = mint,
        associated_token::authority = signer,
        associated_token::token_program = token_program,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,
    
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,

}

pub fn process_withdraw(
    ctx: Context<Withdraw>,
    amount: u64
) -> Result<()> {
    let user : &mut Account<'_, User> = &mut ctx.accounts.user_account;

    // Ensure the user has enough deposited amount to withdraw
    let deposited_value: u64;
    if ctx.accounts.mint.to_account_info().key == &user.usdc_address {
        deposited_value = user.deposited_usdc;
    } else {
        deposited_value = user.deposited_sol;
    }

    // interest check and calculate 

    let time_diff = user.last_updated - Clock::get()?.unix_timestamp;

    let bank = &mut ctx.accounts.bank;

    // formula compound interest 
    // original value * e^(interest rate * times) 
    bank.total_deposits = (bank.total_deposits as f32 * E.powf(bank.interest_rate as f32 * time_diff as f32) ) as u64;

    let value_per_share = bank.total_deposits as f64 / bank.total_deposit_shares as f64;

    let user_value =  deposited_value as f64 / value_per_share;

    if user_value < amount as f64 {
        return Err(ErrorCode::InsufficientFunds.into());
    }



    let transfer_cpi_accounts = TransferChecked {
        from: ctx.accounts.bank_token_account.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.user_token_account.to_account_info(),
        authority: ctx.accounts.bank_token_account.to_account_info(),
    };

    let cpi_program = ctx.accounts.token_program.to_account_info();

    // Create signer seeds for the bank's token account
    let mint_key = ctx.accounts.mint.key();
    // Use the bank's token account bump to derive the signer seeds
    // This assumes that the bank_token_account is initialized with the correct seeds and bump
    let signer_seeds: &[&[&[u8]]] = &[
        &[
            b"treasury",
            mint_key.as_ref(),
            &[ctx.bumps.bank_token_account],
        ],
    ];

    let cpi_ctx = CpiContext::new(cpi_program, transfer_cpi_accounts).with_signer(signer_seeds);

    let decimals = ctx.accounts.mint.decimals;

    token_interface::transfer_checked(
        cpi_ctx,
        amount,
        decimals,
    )?;

    let bank = &mut ctx.accounts.bank;
    let shares_to_remove = (amount as f64 / bank.total_deposit_shares as f64) * bank.total_deposits as f64;

    let user = &mut ctx.accounts.user_account;

    if ctx.accounts.mint.to_account_info().key == &user.usdc_address {
        user.deposited_usdc -= amount;
        user.deposited_usdc_shares -= shares_to_remove as u64;
    } else {
        user.deposited_sol -= amount;
        user.deposited_sol_shares -= shares_to_remove as u64;
    }

    bank.total_deposits -= amount;
    bank.total_deposit_shares -= shares_to_remove as u64;


    Ok(())
}