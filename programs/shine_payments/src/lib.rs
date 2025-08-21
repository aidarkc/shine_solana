use anchor_lang::prelude::*;

declare_id!("92sgkgx7KHpbhQu81mNGHaKa7skJB7esArVdPM7paDSW");


/// Подключаем модуль с полной реализацией.
pub mod investments;
use investments::*; // импортируем всё в корень

/// ==============================================
/// Константы формата / сидов / размеров
/// ==============================================

/// Префикс (seed) для PDA, где храним глобальное состояние выплат.
/// Важно: сид — это просто набор байт; здесь он фиксированный.
pub const PDA_SEED_PREFIX: &[u8] = b"shine_investments_state";

/// Версия формата сериализации нашей структуры состояния.
// pub const INVEST_STATE_FORMAT_V1: u32 = 1; // ← «формат» = 1

/// Значение коэффициента «по умолчанию» при инициализации.
pub const DEFAULT_COEF: u32 = 10; // ← «коэффициент» = 10 при init

/// Кол-во 4-байтовых чисел в состоянии = 7 (см. структуру ниже),
/// значит «голые» данные занимают 7 * 4 = 28 байт.
// pub const PAY_STATE_RAW_LEN_V1: usize = 7 * 4; // 28 байт

/// Ровно столько байт резервируем под PDA-данные.
/// (Можно добавить запас на будущее, но по заданию — только 28.)
pub const PAY_STATE_SPACE: u64 = 50; // просто сделал с запасом PAY_STATE_RAW_LEN_V1 as u64;





#[program]
pub mod shine_payments {
    use super::*;
    // use crate::payments;


    /// init — создаёт PDA и кладёт дефолтное состояние.
    pub fn init(ctx: Context<Init>) -> Result<()> {
        investments::init(ctx) // делегируем в модуль payments
    }

    // TODO: пока только шаблоны вызова основных функций

    /// invest — в начале читает состояние, в конце сохраняет (логика внутри модуля).
    pub fn invest(ctx: Context<UseState>, amount: u64) -> Result<()> {
        investments::invest(ctx, amount) // делегируем
    }

    /// add_bonus — начисление бонусов (обычно от DAO).
    pub fn add_bonus(ctx: Context<UseState>, investor: Pubkey, amount: u64) -> Result<()> {
        investments::add_bonus(ctx, investor, amount) // делегируем
    }

    /// claim — выплата.
    pub fn claim(ctx: Context<UseState>) -> Result<()> {
        investments::claim(ctx) // делегируем
    }


















    ///     ВРЕМЕННАЯ ФУНКЦИЯ      только для тестов и в итоговой версии её не будет
    ///
    /// ===============================
    /// deleteInit — удалить PDA из init и вернуть ренту подписанту
    /// ===============================
    pub fn delete_init(ctx: Context<DeleteInit>) -> Result<()> {
        let program_id = ctx.program_id;

        // PDA по тем же сиду/бампу, что и в init
        let (expected_pda, bump) = Pubkey::find_program_address(&[PDA_SEED_PREFIX], program_id);
        require_keys_eq!(expected_pda, ctx.accounts.state_pda.key(), ErrCode::InvalidPdaAddress);

        // сиды для подписи PDA при assign()
        let seeds: [&[u8]; 2] = [PDA_SEED_PREFIX, &[bump]];

        // Вызов общего утилити-метода: рента уйдёт на счёт подписанта (signer)
        common::utils::delete_pda_return_rent(
            &ctx.accounts.state_pda.to_account_info(),
            &ctx.accounts.signer.to_account_info(),
            program_id,
        )
    }
}
    /// Контекст для deleteInit                         этого тоже в итоге не будет
    #[derive(Accounts)]
    pub struct DeleteInit<'info> {
        /// Подписант транзакции — ПОЛУЧАТЕЛЬ ренты
        #[account(mut)]
        pub signer: Signer<'info>,

        /// Тот самый PDA из init
        /// CHECK: адрес валидируем в хендлере по сид-у
        #[account(mut)]
        pub state_pda: UncheckedAccount<'info>,

        /// Системная программа
        pub system_program: Program<'info, System>,
    }

