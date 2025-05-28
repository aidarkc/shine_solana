use anchor_lang::prelude::*;

/// Основная логика регистрации пользователя:
/// проверка логина и сохранение данных в PDA
pub fn register_user_impl(ctx: Context<RegisterUser>, login: String, pubkey: Pubkey) -> Result<()> {
    if login.len() > 32 {
        return err!(ErrorCode::InvalidLogin);
    }

    for c in login.chars() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return err!(ErrorCode::InvalidLogin);
        }
    }

    let user = &mut ctx.accounts.user_data;
    user.login = login;
    user.pubkey = pubkey;

    msg!("Пользователь зарегистрирован");
    Ok(())
}

/// Аккаунты, участвующие в регистрации
#[derive(Accounts)]
#[instruction(login: String)]
pub struct RegisterUser<'info> {
    /// Подписант, который также платит за создание PDA
    #[account(mut)]
    pub signer: Signer<'info>,

    /// PDA-аккаунт пользователя, создаётся по login
    #[account(
        init,
        payer = signer,
        space = 8 + 4 + 32 + 32 + 100,
        seeds = [b"user", login.as_bytes()],
        bump
    )]
    pub user_data: Account<'info, UserData>,

    /// Системная программа (обязательная для инициализации)
    pub system_program: Program<'info, System>,
}

/// Структура хранения данных пользователя в PDA
#[account]
pub struct UserData {
    pub login: String,
    pub pubkey: Pubkey,
}

/// Ошибки, которые могут возникать при регистрации
#[error_code]
pub enum ErrorCode {
    #[msg("Неверный логин: допускаются только маленькие буквы, цифры и _")]
    InvalidLogin,
}
