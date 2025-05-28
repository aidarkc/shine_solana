use anchor_lang::prelude::*;

mod say_hello;

mod register_user;


mod user_module;

use say_hello::*;
use register_user::*;
use user_module::*;


declare_id!("BmCgGmQbSjkE6Zg8WAwhxDMNHiTknMYqTF4ZVMrPdTpz");

#[program]
pub mod hello_solana {
    use super::*;

    // Вызов функции из say_hello.rs
    pub fn say_hello(ctx: Context<SayHello>) -> Result<()> {
        say_hello_impl(ctx)
    }

    // Вызов функции из register_user.rs
    pub fn register_user(ctx: Context<RegisterUser>, login: String, pubkey: Pubkey) -> Result<()> {
        register_user_impl(ctx, login, pubkey)
    }


/*
    /// Entry-point, который будет виден в IDL и вызываться с фронта.
    /// Из него просто пробрасываем вызов в реальную логику.
    #[derive(Accounts)]
    #[instruction(login: String, account_size: u32)]
    pub struct RegisterUser2Entry<'info> {
        // Тот же самый набор аккаунтов, что и в user_module::RegisterUser2
        #[account(mut)]
        signer: Signer<'info>,

        #[account(mut, seeds = [USER_COUNT_PDA_SEED], bump)]
        user_count: UncheckedAccount<'info>,

        #[account(mut)]
        search_by_name: UncheckedAccount<'info>,

        #[account(mut)]
        big_user_pda: UncheckedAccount<'info>,

        #[account(mut)]
        search_by_id: UncheckedAccount<'info>,

        system_program: Program<'info, System>,
    }
*/
    
    
    /// Обработчик Anchor-инструкции.
    /// `pubkey` можно передать отдельным аргументом, либо брать из signer.key().
    pub fn do_register_user2(ctx: Context<RegisterUser2>, login: String, pubkey: Pubkey, account_size: u32, ) -> Result<()> {
        // просто делегируем в реальную функцию
        let inner: Context<RegisterUser2> = ctx.into();     // преобразуем в ожидаемый тип
        register_user2(inner, login, pubkey, account_size)
    }
}
