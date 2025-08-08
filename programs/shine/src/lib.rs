use anchor_lang::prelude::*;



mod utils;

// Подключаем модуль utils
use utils::*;        // Импортируем все функции и структуры из него


declare_id!("5dFcWDNp42Xn9Vv4oDMJzM4obBJ8hvDuAtPX54fT5L3t");


#[program]
pub mod shine {
    use super::*;



///---------------------------------------------------------------
/// Дальше по делу :)
/// --------------------------------------------------------------



    /// Вызов register_user_step_one — расширенная регистрация
    pub fn register_user_step_one(
        ctx: Context<RegisterUserStepOne>,
        login: String,
        pubkey: Pubkey,
    ) -> Result<()> {
        utils::register_user_step_one(ctx, login, pubkey)
    }



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
    
    /// Регистрация пользователя с одним устройством
    ///
    /// Выполняет регистрацию нового пользователя:
    /// - Проверяет логин, валидность PDA и уникальность
    /// - Переводит комиссию 0.01 SOL
    /// - Увеличивает счётчик пользователей
    /// - Создаёт два PDA:
    ///     1. по логину (UserByLogin)
    ///     2. по ID (UserById), выбирая один из пяти возможных адресов
    ///
    /// Требует:
    /// - signer: аккаунт-подписант, равный переданному pubkey
    /// - user_counter: PDA со счётчиком
    /// - user_by_login_pda: PDA по логину
    /// - id_pda_1..5: возможные PDA по ID (из которых будет выбран подходящий)
    /// - system_program и fee_receiver — стандартные
    pub fn register_user_with_one_dev(
        ctx: Context<RegisterUserWithOneDev>,
        login: String,
        pubkey: Pubkey,              // подпись пользователя (должна быть signer)
        device_sign_pubkey: Pubkey, // подпись устройства
        device_x25519_pubkey: Pubkey, // X25519 ключ для шифрования
    ) -> Result<()> {
        utils::register_user_with_one_dev(
            ctx,
            login,
            pubkey,
            device_sign_pubkey,
            device_x25519_pubkey,
        )
    }
}
