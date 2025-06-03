use anchor_lang::prelude::*;

mod say_hello;

mod register_user;



use say_hello::*;
use register_user::*;


mod utils;
// Подключаем модуль utils
use utils::*;        // Импортируем все функции и структуры из него


declare_id!("BmCgGmQbSjkE6Zg8WAwhxDMNHiTknMYqTF4ZVMrPdTpz");


#[program]
pub mod hello_solana {
    use super::*;



    /// Тестовая функция — проксирует вызов в модуль `utils`
    pub fn test_utils(ctx: Context<TestContext>, extra_pubkey: Pubkey,
        number: u64, note: String,
        str_array: Vec<String>,
    ) -> Result<()> {
        test(ctx, extra_pubkey, number, note, str_array)
    }



    // Вызов функции из say_hello.rs
    pub fn say_hello(ctx: Context<SayHello>) -> Result<()> {
        say_hello_impl(ctx)
    }

    // Вызов функции из register_user.rs
    pub fn register_user(ctx: Context<RegisterUser>, login: String, pubkey: Pubkey) -> Result<()> {
        register_user_impl(ctx, login, pubkey)
    }

    
    
    /// Расширенная регистрация пользователя
    // pub fn do_register_user2(ctx: Context<RegisterUser2>, login: String, pubkey: Pubkey, account_size: u32) -> Result<()> {
    //     register_user2(ctx.into(), login, pubkey, account_size)
    // }

    /// Одноразовая инициализация счётчика пользователей
    pub fn initialize_user_counter(ctx: Context<InitUserCounter>) -> Result<()> {
        // Вызов внутренней логики из утилит
        utils::initialize_user_counter(
            &ctx.accounts.counter_pda,
            &ctx.accounts.signer,
            &ctx.accounts.system_program,
            ctx.program_id,
        )
    }
}
