use anchor_lang::prelude::*;

use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use common::utils::*; // тянем общие PDA-хелперы из programs/common



/// Утилита чтения структуры из PDA: читает байты и десериализует.
/// Возвращает ошибку, если данных нет/пустые/неверный формат.
fn read_state_from_pda(pda: &AccountInfo) -> Result<InvestState> {
    let raw = safe_read_pda(pda);                                   // ← берём Vec<u8> (или пустой)
    require!(!raw.is_empty(), ErrCode::EmptyPdaData);               // ← пусто — ошибка
    let st = deserialize_invest_state(&raw)?;                          // ← десериализуем по формату
    require!(st.format == INVEST_STATE_FORMAT_V1, ErrCode::UnsupportedFormat); // ← проверяем версию
    Ok(st)
}

/// Утилита записи структуры в PDA: сериализует и пишет.
/// Важно: сам аккаунт уже должен существовать и быть #[account(mut)].
fn write_state_to_pda(pda: &AccountInfo, s: &InvestState) -> Result<()> {
    let raw = serialize_invest_state_v1(s); // ← 28 байт
    write_to_pda(pda, &raw)              // ← записываем в начало data
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

// pub const ALLOWED_INIT_CALLER: Pubkey =
//     Pubkey::from_str("FUc28vNixp7F3nnkpGVt6nuJbgvJ4429v4B5wS52Df6P")
//         .unwrap();

/// ------------------------------------------
/// init: создаёт PDA и записывает в него дефолтное состояние.
/// format = 1, coef = 10, остальные поля = 0.
/// ------------------------------------------
pub fn init(ctx: Context<Init>) -> Result<()> {
    let program_id = ctx.program_id;                              // ← адрес этой программы

    // 1. Проверка что вызывает именно разрешённый ключ
    /* todo   пока все могут вызыватьно вообще можно добавить  - и вообще вопрос кто будет это вызывать ? :)
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
    
    let pda_key_expected = Pubkey::find_program_address(&[PDA_SEED_PREFIX], program_id).0; // ← вычисляем PDA
    require_keys_eq!(
        pda_key_expected,
        ctx.accounts.state_pda.key(),
        ErrCode::InvalidPdaAddress
    ); // ← убеждаемся, что нам подали именно правильный PDA

    // Конструируем дефолтную структуру состояния.
    let state = InvestState {
        format: INVEST_STATE_FORMAT_V1,  // ← 1
        coef: DEFAULT_COEF,           // ← 10
        q1_tokens: 0,                 // ← нули
        sum1_bonus: 0,
        q1_paid_tokens: 0,
        sum1_paid_bonus: 0,
    };


    // Сериализуем в 28 байт.
    let data = serialize_invest_state_v1(&state);

    // Для подписи PDA нужен bump; здесь получим (ключ, bump).
    let (_pda_key, bump) = Pubkey::find_program_address(&[PDA_SEED_PREFIX], program_id);

    // Сиды для invoke_signed: [seed, bump]
    let seeds: [&[u8]; 2] = [PDA_SEED_PREFIX, &[bump]];

    // Создаём и сразу записываем, арендный минимум оплачивает payer.
    create_and_write_pda(
        &ctx.accounts.state_pda.to_account_info(),   // куда пишем
        &ctx.accounts.payer.to_account_info(),       // кто платит
        &ctx.accounts.system_program.to_account_info(),
        program_id,
        &seeds,
        data,
        PAY_STATE_SPACE,                          // 28 байт
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
/// По заданию: читаем в начале, сохраняем в конце.
/// ------------------------------------------
pub fn add_bonus(ctx: Context<UseState>, _investor: Pubkey, _coef: u64) -> Result<()> {
    // 1) читаем
    let mut st = read_state_from_pda(&ctx.accounts.state_pda.to_account_info())?;

    // --- здесь можно добавить логику корректировки sum1_bonus и т.п. ---
    let _ = (&mut st, _investor, _coef); // заглушка, чтобы не было warning

    // 2) сохраняем
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
/// Почему 6? Потому что у нас 6 полей по 4 байта (u32).
pub const INVEST_STATE_RAW_LEN_V1: usize = 24; // байт

/// ================================
/// ОПИСАНИЕ СТРУКТУРЫ СОСТОЯНИЯ (V1)
/// ================================
/// Мы храним глобальные агрегаты по выплатам в одном PDA.
/// Каждый элемент — 32-битное беззнаковое число (u32), Little Endian.
///
/// ПОЛЯ:
///  1) format          — версия формата (всегда 1 для этого кода)
///  2) coef            — коэффициент (по умолчанию 10, но можно менять)
///  3) q1_tokens       — сколько токенов стоит в очереди на выплату (1-я очередь)
///  4) sum1_bonus      — общая сумма «бонусов», которые нужно выплатить по 1-й очереди
///  5) q1_paid_tokens  — сколько токенов уже выплачено по 1-й очереди (счётчик выполненного)
///  6) sum1_paid_bonus — какая сумма «бонусов» уже выплачена по 1-й очереди
///
/// Итого: 6 полей * 4 байта = 24 байта.
#[derive(Clone, Copy, Debug, Default)]
pub struct InvestState {
    /// Версия формата, должна быть INVEST_STATE_FORMAT_V1 (то есть 1).
    pub format: u32,

    /// Текущий коэффициент (ваша бизнес-логика; например, 10 при инициализации).
    pub coef: u32,

    /// Кол-во токенов в 1-й очереди, ожидающих выплаты.
    pub q1_tokens: u32,

    /// Сумма бонусов, подлежащая выплате по 1-й очереди (ещё не выплачено).
    pub sum1_bonus: u32,

    /// Сколько токенов уже выплачено по 1-й очереди (накопительный счётчик).
    pub q1_paid_tokens: u32,

    /// Какая сумма бонусов уже выплачена по 1-й очереди (накопительный счётчик).
    pub sum1_paid_bonus: u32,
}









/// ========================================
/// СЕРИАЛИЗАЦИЯ (структура -> массив байт)
/// ========================================
/// Мы вручную упаковываем каждое поле в 4 байта в порядке Little Endian.
/// ПОЛНЫЙ РАЗМЕР: ровно 24 байта.
/// ПОРЯДОК ПОЛЕЙ (по 4 байта каждое):
///   [0..4)  format
///   [4..8)  coef
///   [8..12) q1_tokens
///   [12..16) sum1_bonus
///   [16..20) q1_paid_tokens
///   [20..24) sum1_paid_bonus
pub fn serialize_invest_state_v1(s: &InvestState) -> Vec<u8> {
    // Для людей без опыта в Rust:
    // Vec<u8> — это "динамический массив байт".
    // Мы заранее резервируем 24 байта, чтобы не делать лишних перераспределений.
    let mut out = Vec::with_capacity(INVEST_STATE_RAW_LEN_V1);

    // Нормируем версию: даже если в поле format «что-то другое»,
    // мы пишем именно константу версии, чтобы на чейне хранилась корректная метка формата.
    out.extend_from_slice(&INVEST_STATE_FORMAT_V1.to_le_bytes()); // [0..4)

    // Далее — остальные поля как есть, по 4 байта LE каждое.
    out.extend_from_slice(&s.coef.to_le_bytes());            // [4..8)
    out.extend_from_slice(&s.q1_tokens.to_le_bytes());       // [8..12)
    out.extend_from_slice(&s.sum1_bonus.to_le_bytes());      // [12..16)
    out.extend_from_slice(&s.q1_paid_tokens.to_le_bytes());  // [16..20)
    out.extend_from_slice(&s.sum1_paid_bonus.to_le_bytes()); // [20..24)

    // Итоговая длина должна быть ровно 24 байта.
    debug_assert_eq!(out.len(), INVEST_STATE_RAW_LEN_V1);
    out
}

/// ===========================================
/// ДЕСЕРИАЛИЗАЦИЯ (массив байт -> структура)
/// ===========================================
/// На вход подаём срез байт `data`, ожидаем минимум 24 байта.
/// 1) Проверяем, что данных хватает.
/// 2) Считываем первые 4 байта как `format` и убеждаемся, что это версия 1.
/// 3) Последовательно читаем остальные 5 чисел по 4 байта (LE) каждое.
/// 4) Возвращаем заполненную структуру.
pub fn deserialize_invest_state(data: &[u8]) -> Result<InvestState> {
    // 1) Проверяем длину. Если меньше 24 байт — данных недостаточно.
    if data.len() < INVEST_STATE_RAW_LEN_V1 {
        return Err(error!(ErrCode::DeserializationError));
    }

    // Вспомогательная функция: безопасно читает 4 байта как u32 в Little Endian
    // из указанного диапазона [start..start+4).
    fn read_u32_le(slice: &[u8], start: usize) -> u32 {
        // Здесь используем get(..) + try_into(), чтобы не паниковать при неверных индексах.
        let bytes: [u8; 4] = slice[start..start + 4]
            .try_into()
            .expect("slice has enough length due to pre-check");
        u32::from_le_bytes(bytes)
    }

    // 2) Читаем и проверяем версию формата.
    let format = read_u32_le(data, 0);
    if format != INVEST_STATE_FORMAT_V1 {
        // Если формат другой — значит это не поддерживаемая версия.
        return Err(error!(ErrCode::UnsupportedFormat));
    }

    // 3) Читаем остальные поля по 4 байта.
    let coef            = read_u32_le(data, 4);
    let q1_tokens       = read_u32_le(data, 8);
    let sum1_bonus      = read_u32_le(data, 12);
    let q1_paid_tokens  = read_u32_le(data, 16);
    let sum1_paid_bonus = read_u32_le(data, 20);

    // 4) Собираем структуру и возвращаем её.
    Ok(InvestState {
        format,
        coef,
        q1_tokens,
        sum1_bonus,
        q1_paid_tokens,
        sum1_paid_bonus,
    })
}
