use anchor_lang::prelude::*;

declare_id!("6Hes38UKFGF8cfQDQDVWoMGcSzGMUAgamWG31hCVhyPY");


/// Подключаем модуль с полной реализацией.
pub mod investments;
use investments::*; // импортируем всё в корень

// === модуль NFT ===
pub mod nft;

// ==============================================
// Константы формата / сидов / размеров
// ==============================================

/// Префикс (seed) для PDA, где храним глобальное состояние выплат.
/// Важно: сид — это просто набор байт; здесь он фиксированный.
pub const PDA_SEED_PREFIX: &[u8] = b"shine_investments_state";

/// Значение коэффициента «по умолчанию» при инициализации.
pub const DEFAULT_COEF: u32 = 10; // ← «коэффициент» = 10 при init

/// Ровно столько байт резервируем под PDA-данные.
/// (Можно добавить запас на будущее, но по заданию — только 28.)
pub const PAY_STATE_SPACE: u64 = 50; // просто сделал с запасом

// ==============================================
// Программа
// ==============================================

#[program]
pub mod shine_payments {
    use super::*;
    // Явно подтягиваем типы и функции, чтобы не было путаницы после предыдущих ошибок парсера
    use crate::investments::{Init, UseState};
    use crate::investments::{
        add_bonus as inv_add_bonus, claim as inv_claim, init as inv_init, invest as inv_invest,
        ErrCode,
    };

    /// init — создаёт PDA и кладёт дефолтное состояние.
    pub fn init(ctx: Context<Init>) -> Result<()> {
        inv_init(ctx)
    }

    /// invest — в начале читает состояние, в конце сохраняет (логика внутри модуля).
    pub fn invest(ctx: Context<UseState>, amount: u64) -> Result<()> {
        inv_invest(ctx, amount)
    }

    /// add_bonus — начисление бонусов (обычно от DAO).
    /// Для NFT используем расширенный контекст AddBonusCtx (с аккаунтами коллекции и т.п.).
    pub fn add_bonus(ctx: Context<AddBonusCtx>, investor: Pubkey, amount: u64) -> Result<()> {
        inv_add_bonus(ctx, investor, amount)
    }

    /// claim — выплата.
    pub fn claim(ctx: Context<UseState>) -> Result<()> {
        inv_claim(ctx)
    }

    /// ВРЕМЕННАЯ ФУНКЦИЯ только для тестов (в итоговой версии её не будет):
    /// deleteInit — удалить PDA из init и вернуть ренту подписанту.
    pub fn delete_init(ctx: Context<DeleteInit>) -> Result<()> {
        let program_id = ctx.program_id;

        // PDA по тем же сид/бамп, что и в init
        let (expected_pda, _bump) = Pubkey::find_program_address(&[PDA_SEED_PREFIX], program_id);
        require_keys_eq!(
            expected_pda,
            ctx.accounts.state_pda.key(),
            ErrCode::InvalidPdaAddress
        );

        // Рента уйдёт на счёт подписанта (signer)
        common::utils::delete_pda_return_rent(
            &ctx.accounts.state_pda.to_account_info(),
            &ctx.accounts.signer.to_account_info(),
            program_id,
        )
    }
}

// ==============================================
// Контексты вне #[program]
// ==============================================

/// Контекст для deleteInit (временный для тестов)
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

/// Контекст для add_bonus: полный набор аккаунтов для операций с NFT и коллекцией.
/// (Комменты по стилю проекта оставлены.)
#[derive(Accounts)]
pub struct AddBonusCtx<'info> {
    /// Любой платящий/подписант (в реальном коде — свои проверки).
    #[account(mut)]
    pub signer: Signer<'info>,

    /// Тот же PDA с состоянием (должен уже существовать).
    /// CHECK: проверяется вручную по адресу
    #[account(mut)]
    pub state_pda: UncheckedAccount<'info>,

    // --- аккаунты минтимого NFT ---
    /// Mint создаваемого NFT (должен быть создан заранее: decimals=0, mint_authority=signer, freeze_authority=signer)
    /// CHECK
    #[account(mut)]
    pub mint_pda: UncheckedAccount<'info>,

    /// ATA получателя (может быть предсоздан тестом)
    /// CHECK
    #[account(mut)]
    pub recipient_ata: UncheckedAccount<'info>,
    /// Владелец ATA (инвестор)
    /// CHECK
    pub recipient_owner: UncheckedAccount<'info>,

    // --- аккаунты коллекции (уже созданной заранее) ---
    /// CHECK
    pub collection_mint: UncheckedAccount<'info>,
    /// CHECK
    #[account(mut)]
    pub collection_metadata_pda: UncheckedAccount<'info>,
    /// CHECK
    #[account(mut)]
    pub collection_master_edition_pda: UncheckedAccount<'info>,
    /// Апдейтер коллекции (update authority)
    pub collection_update_authority: Signer<'info>,

    // --- metadata + master edition для создаваемого NFT ---
    /// CHECK
    #[account(mut)]
    pub metadata_pda: UncheckedAccount<'info>,
    /// CHECK
    #[account(mut)]
    pub master_edition_pda: UncheckedAccount<'info>,

    // --- программы ---
    /// CHECK: проверяется по ID внутри nft.rs
    pub token_metadata_program: UncheckedAccount<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
}
