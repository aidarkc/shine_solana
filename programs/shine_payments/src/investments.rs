use anchor_lang::prelude::*;

use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use common::utils::*; // тянем общие PDA-хелперы из programs/common

// === добавлено: используем наш NFT-модуль ===
use crate::nft::{CreateNftParams, create_nft_with_freeze};
// ============================================

/// Утилита чтения структуры из PDA: читает байты и десериализует.
/// Возвращает ошибку, если данных нет/пустые/неверный формат.
fn read_state_from_pda(pda: &AccountInfo) -> Result<InvestState> {
    let raw = safe_read_pda(pda);                         // ← берём Vec<u8> (или пустой)
    require!(!raw.is_empty(), ErrCode::EmptyPdaData);     // ← пусто — ошибка
    let st = deserialize_invest_state(&raw)?;             // ← десериализуем по формату
    require!(st.format == INVEST_STATE_FORMAT_V1, ErrCode::UnsupportedFormat); // ← проверяем версию
    Ok(st)
}

/// Утилита записи структуры в PDA: сериализует и пишет.
/// Важно: сам аккаунт уже должен существовать и быть #[account(mut)].
fn write_state_to_pda(pda: &AccountInfo, s: &InvestState) -> Result<()> {
    let raw = serialize_invest_state_v1(s); // ← 24 байта
    write_to_pda(pda, &raw)                 // ← записываем в начало data
}

/// ==============================================
/// Контексты инструкций (минимально необходимые)
/// ==============================================

/// init: создаём PDA и кладём в него PayStateV1 {format=1, coef=10, ...0}
#[derive(Accounts)]
pub struct Init<'info> {
    /// Плательщик аренды за PDA; подписант транзакции.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Наш PDA (с произвольным типом, чтобы работать через AccountInfo).
    /// Проверку адреса делаем в handler (по seed + bump), чтобы избежать подмены.
    /// CHECK: проверяется вручную по адресу
    #[account(mut)]
    pub state_pda: UncheckedAccount<'info>,

    /// Системная программа.
    pub system_program: Program<'info, System>,
}

/// Общие аккаунты для invest/add_bonus/claim:
/// Везде просто читаем/пишем одно и то же состояние из того же PDA.
#[derive(Accounts)]
pub struct UseState<'info> {
    /// Любой платящий/подписант (в реальном коде — свои проверки).
    pub signer: Signer<'info>,

    /// Тот же PDA с состоянием (должен уже существовать).
    /// CHECK: проверяется вручную по адресу
    #[account(mut)]
    pub state_pda: UncheckedAccount<'info>,

    /// Системная программа (на всякий случай; может не понадобиться).
    pub system_program: Program<'info, System>,
}

/// ==============================================
/// Программа
/// ==============================================

use super::*;
use anchor_lang::prelude::*;


/// ------------------------------------------
/// init: создаёт PDA и записывает в него дефолтное состояние.
/// format = 1, coef = 10, остальные поля = 0.
/// ------------------------------------------
pub fn init(ctx: Context<Init>) -> Result<()> {
    let program_id = ctx.program_id; // ← адрес этой программы

    // 1. Проверка что вызывает именно разрешённый ключ
    /* todo   пока все могут вызыватьно                                         !! но в итоге будет добавленна проверка что бы только дао могло вызвать эту функцию один раз
    require_keys_eq!(
        ctx.accounts.payer.key(),
        ALLOWED_INIT_CALLER,
        ErrCode::InvalidSigner
    );
*/

    // 2. Проверка что PDA ещё не создан
    if ctx.accounts.state_pda.data_len() > 0 && ctx.accounts.state_pda.owner != &System::id() {
        return Err(error!(ErrCode::PdaAlreadyExists));
    }
    
    // 2. Ещё раз Проверка что PDA ещё не создан  
    if ctx.accounts.state_pda.owner != &System::id()
        || ctx.accounts.state_pda.lamports() > 0
    {
        // Если аккаунт уже создан и не пустой
        return Err(error!(ErrCode::PdaAlreadyExists));
    }
    
    let pda_key_expected = Pubkey::find_program_address(&[crate::PDA_SEED_PREFIX], program_id).0; // ← вычисляем PDA
    require_keys_eq!(
        pda_key_expected,
        ctx.accounts.state_pda.key(),
        ErrCode::InvalidPdaAddress
    ); // ← убеждаемся, что нам подали именно правильный PDA

    // Конструируем дефолтную структуру состояния.
    let state = InvestState {
        format: INVEST_STATE_FORMAT_V1,  // ← 1
        coef: crate::DEFAULT_COEF,       // ← 10
        q1_tokens: 0,                    // ← нули
        sum1_bonus: 0,
        q1_paid_tokens: 0,
        sum1_paid_bonus: 0,
    };

    // Сериализуем в 24 байта.
    let data = serialize_invest_state_v1(&state);

    // Для подписи PDA нужен bump; здесь получим (ключ, bump).
    let (_pda_key, bump) = Pubkey::find_program_address(&[crate::PDA_SEED_PREFIX], program_id);

    // Сиды для invoke_signed: [seed, bump]
    let seeds: [&[u8]; 2] = [crate::PDA_SEED_PREFIX, &[bump]];

    // Создаём и сразу записываем, арендный минимум оплачивает payer.
    create_and_write_pda(
        &ctx.accounts.state_pda.to_account_info(),   // куда пишем
        &ctx.accounts.payer.to_account_info(),       // кто платит
        &ctx.accounts.system_program.to_account_info(),
        program_id,
        &seeds,
        data,
        crate::PAY_STATE_SPACE,                      // резерв с запасом
    )?;

    Ok(())
}

/// ------------------------------------------
/// invest: «внос инвестиций».
/// По заданию: в начале читаем состояние, в конце сохраняем.
/// (Здесь логика модификации не задана — оставляем как заглушку.)
/// ------------------------------------------
pub fn invest(ctx: Context<UseState>, _amount: u64) -> Result<()> {
    // 1) читаем
    let mut st = read_state_from_pda(&ctx.accounts.state_pda.to_account_info())?; // ← PayStateV1

    // --- тут можно модифицировать st по твоей бизнес-логике ---
    // Например, ничего не меняем сейчас (заглушка).
    let _ = &mut st; // чтоб компилятор не ругался, если пока не используем

    // 2) сохраняем
    write_state_to_pda(&ctx.accounts.state_pda.to_account_info(), &st)?;
    Ok(())
}

/// ------------------------------------------
/// add_bonus: «начисление бонусов» (обычно вызывать от DAO).
/// По заданию: читаем в начале, создаём/добавляем NFT в очередь, сохраняем в конце.
/// Для операций с NFT используем расширенный контекст AddBonusCtx (см. lib.rs).
/// ------------------------------------------
pub fn add_bonus(ctx: Context<crate::AddBonusCtx>, investor: Pubkey, amount: u64) -> Result<()> {
    // 1) читаем состояние
    let mut st = read_state_from_pda(&ctx.accounts.state_pda.to_account_info())?;

    // 2) создаём/добавляем NFT через модуль nft (создание metadata, mint 1, freeze, master edition, verify)
    let next_index = st.q1_tokens as u64 + 1;
    let params = CreateNftParams {
        name: format!("Bonus #{}", next_index),
        symbol: "BN".to_string(),
        uri: "https://example.com/nft.json".to_string(), // заглушка для devnet-теста
        index: next_index,
        recipient: investor,
    };

    // ВАЖНО: mint_pda должен быть создан ТЕСТОМ заранее с decimals=0, mint_authority=signer, freeze_authority=signer.
    create_nft_with_freeze(&ctx, params)?;

    // 3) обновляем агрегаты очереди (минимально: увеличим счётчик и сумму бонусов)
    st.q1_tokens = st.q1_tokens.saturating_add(1);
    let add = u32::try_from(core::cmp::min(amount, u64::from(u32::MAX))).unwrap_or(u32::MAX);
    st.sum1_bonus = st.sum1_bonus.saturating_add(add);

    // 4) сохраняем
    write_state_to_pda(&ctx.accounts.state_pda.to_account_info(), &st)?;
    Ok(())
}

/// ------------------------------------------
/// claim: «выплата».
/// По заданию: читаем в начале, сохраняем в конце.
/// ------------------------------------------
pub fn claim(ctx: Context<UseState>) -> Result<()> {
    // 1) читаем
    let mut st = read_state_from_pda(&ctx.accounts.state_pda.to_account_info())?;

    // --- тут твоя логика списаний/выплат ---
    let _ = &mut st; // заглушка

    // 2) сохраняем
    write_state_to_pda(&ctx.accounts.state_pda.to_account_info(), &st)?;
    Ok(())
}





//todo





/// ==============================================
/// Коды ошибок (берём из твоего блока; можно расширять)
/// ==============================================

#[error_code]
pub enum ErrCode {
    /// Система уже инициализирована и не может быть инициализирована повторно!
    #[msg("Система уже инициализирована и не может быть инициализирована повторно!")]
    SystemAlreadyInitialized = 1000,

    #[msg("PDA не содержит данных или не инициализирован")]
    EmptyPdaData = 1002,

    #[msg("Пользователь уже зарегистрирован")]
    UserAlreadyExists = 1003,

    #[msg("Некорректный логин")]
    InvalidLogin = 1004,

    #[msg("Не совпадает PDA адрес")]
    InvalidPdaAddress = 1006,

    #[msg("Формат данных не поддерживается")]
    UnsupportedFormat = 1011,

    #[msg("Ошибка при десериализации")]
    DeserializationError = 1012,

    /// PDA уже существует, создание невозможно
    #[msg("PDA-аккаунт уже существует и не может быть создан повторно.")]
    PdaAlreadyExists = 1009,

    #[msg("Подписавший не совпадает с ожидаемым пользователем (временное ограничение)")]
    InvalidSigner = 1005,

    /// Не получилось создать пользователя
    #[msg("Не получилось создать пользователя, система уже перегружена, попробуйте позже!")]
    NoSuitableIdPda = 1010,
}





use anchor_lang::prelude::*;

/// ================================
/// КОНСТАНТЫ ФОРМАТА / ДЛИНЫ ДАННЫХ
/// ================================

/// Версия формата хранения состояния.
/// Мы жёстко фиксируем «1», чтобы код мог отличать будущие версии.
pub const INVEST_STATE_FORMAT_V1: u32 = 1;

/// Сырые данные состояния V1 занимают ровно 6 * 4 = 24 байта.
pub const INVEST_STATE_RAW_LEN_V1: usize = 24; // байт

/// ================================
/// ОПИСАНИЕ СТРУКТУРЫ СОСТОЯНИЯ (V1)
/// ================================
#[derive(Clone, Copy, Debug, Default)]
pub struct InvestState {
    pub format: u32,
    pub coef: u32,
    pub q1_tokens: u32,
    pub sum1_bonus: u32,
    pub q1_paid_tokens: u32,
    pub sum1_paid_bonus: u32,
}

/// ========================================
/// СЕРИАЛИЗАЦИЯ (структура -> массив байт)
/// ========================================
pub fn serialize_invest_state_v1(s: &InvestState) -> Vec<u8> {
    let mut out = Vec::with_capacity(INVEST_STATE_RAW_LEN_V1);
    out.extend_from_slice(&INVEST_STATE_FORMAT_V1.to_le_bytes()); // [0..4)
    out.extend_from_slice(&s.coef.to_le_bytes());            // [4..8)
    out.extend_from_slice(&s.q1_tokens.to_le_bytes());       // [8..12)
    out.extend_from_slice(&s.sum1_bonus.to_le_bytes());      // [12..16)
    out.extend_from_slice(&s.q1_paid_tokens.to_le_bytes());  // [16..20)
    out.extend_from_slice(&s.sum1_paid_bonus.to_le_bytes()); // [20..24)
    debug_assert_eq!(out.len(), INVEST_STATE_RAW_LEN_V1);
    out
}

/// ===========================================
/// ДЕСЕРИАЛИЗАЦИЯ (массив байт -> структура)
/// ===========================================
pub fn deserialize_invest_state(data: &[u8]) -> Result<InvestState> {
    if data.len() < INVEST_STATE_RAW_LEN_V1 {
        return Err(error!(ErrCode::DeserializationError));
    }
    fn read_u32_le(slice: &[u8], start: usize) -> u32 {
        let bytes: [u8; 4] = slice[start..start + 4]
            .try_into()
            .expect("slice has enough length due to pre-check");
        u32::from_le_bytes(bytes)
    }
    let format = read_u32_le(data, 0);
    if format != INVEST_STATE_FORMAT_V1 {
        return Err(error!(ErrCode::UnsupportedFormat));
    }
    let coef            = read_u32_le(data, 4);
    let q1_tokens       = read_u32_le(data, 8);
    let sum1_bonus      = read_u32_le(data, 12);
    let q1_paid_tokens  = read_u32_le(data, 16);
    let sum1_paid_bonus = read_u32_le(data, 20);

    Ok(InvestState { format, coef, q1_tokens, sum1_bonus, q1_paid_tokens, sum1_paid_bonus })
}
