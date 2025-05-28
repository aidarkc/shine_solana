use anchor_lang::prelude::*;

/// Простая функция, которая выводит лог в консоль
pub fn say_hello_impl(_ctx: Context<SayHello>) -> Result<()> {
    msg!("Привет, Solana от Айдара!");
    Ok(())
}

/// Структура аккаунтов, участвующих в say_hello
#[derive(Accounts)]
pub struct SayHello<'info> {
    /// Подписант (signer), которого просто логируем
    #[account(mut)]
    pub signer: Signer<'info>,
}
