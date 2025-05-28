//! Модуль расширенной регистрации пользователя (register_user2)
//! ----------------------------------------------------------------
//! Версия без автосериализации Anchor: все данные в PDA пишутся как
//! «сырой» массив байт. В начале каждого PDA находится **4‑байтный
//! дескриптор формата** (сейчас всегда `1`). Далее данные располагаются
//! строго по описанной ниже схеме.
//!
//! ----------------------------------------------------------------
//! СОДЕРЖАНИЕ МОДУЛЯ
//! ----------------------------------------------------------------
//! 1. Константы и префиксы PDA.
//! 2. Пользовательские ошибки (ErrorCodeNew).
//! 3. Структуры данных (для внутренних расчётов, в PDA они лежат в виде
//!    байтов).
//! 4. Функции сериализации / десериализации.
//! 5. Валидация логина, ключа и размера PDA.
//! 6. Один раз вызываемая инструкция `init_system` – создаёт счётчик
//!    пользователей и записывает `[u32 descriptor, u64 count]`.
//! 7. Инструкция `register_user2` – расширенная регистрация пользователя.
//! 8. Вспомогательные функции чтения адреса BigUser PDA по имени / id.
//!
//! Всё снабжено подробными комментариями на русском.
//!
//! ----------------------------------------------------------------
//! ПОДКЛЮЧЕНИЕ
//! ----------------------------------------------------------------
//! Добавьте в `lib.rs`:
//! ```
//! mod user_module;
//! pub use user_module::{init_system, register_user2};
//! ```
//! ----------------------------------------------------------------

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock::Clock, program::invoke_signed, system_instruction};

// ------------------------------------------------------------------
//                            Константы
// ------------------------------------------------------------------

/// Общий дескриптор формата всех PDA – 4‑байтное little‑endian число.
pub const FORMAT_DESCRIPTOR: u32 = 1;

/// seed PDA счётчика пользователей
pub const USER_COUNT_PDA_SEED: &[u8] = b"user_count";
/// Префикс PDA «больших» аккаунтов
pub const BIG_USER_PDA_PREFIX: &[u8] = b"big_user";
/// Префикс поискового PDA по имени
pub const SEARCH_NAME_PDA_PREFIX: &[u8] = b"search_name";
/// Префикс поискового PDA по id
pub const SEARCH_ID_PDA_PREFIX: &[u8] = b"search_id";

/// Размер поля `reserved` внутри BigUser PDA (можно увеличивать для
/// будущих секций данных).
const RESERVED_SIZE: usize = 1024;

// Минимальный реальный объём пользовательских данных без резервов
const MIN_BIG_USER_DATA_SIZE: usize = 4 /*descr*/ + 8 + 1 + 32 + 32 + 8 + 8;

// ------------------------------------------------------------------
//                              Ошибки
// ------------------------------------------------------------------

#[error_code]
pub enum ErrorCodeNew {
    #[msg("Неверный формат имени пользователя: допускаются только a-z, 0-9 и _ (до 32 символов)")]
    InvalidLoginFormat,

    #[msg("Пользователь с таким именем уже существует")]
    UserAlreadyExists,

    #[msg("Имя является платным премиум-именем – требуется отдельная оплата")]
    PremiumName,

    #[msg("Неверный публичный ключ (должен состоять из 32 байт)")]
    InvalidPubkey,

    #[msg("Неверный или неподдерживаемый размер PDA (должно быть 200‑4000 байт)")]
    InvalidAccountSize,
}

// ------------------------------------------------------------------
//                       Структуры (in‑memory)
// ------------------------------------------------------------------

/// Счётчик пользователей (u64)
pub struct UserCountRaw {
    pub count: u64,
}

/// Поисковый PDA (имя/id → адрес BigUser)
pub struct SearchIndexRaw {
    pub big_user: Pubkey,
}

/// Полные данные пользователя (BigUser PDA)
pub struct BigUserData {
    pub id: u64,
    pub login_len: u8,
    pub login: [u8; 32],
    pub pubkey: Pubkey,
    pub created_at: i64,
    pub updated_at: i64,
    pub reserved: [u8; RESERVED_SIZE],
}

impl BigUserData {
    /// Минимальный объём среза, необходимый для сериализации (без учёта
    /// желаемого пользователем `account_size`).
    pub const fn byte_len() -> usize {
        4 + 8 + 1 + 32 + 32 + 8 + 8 + RESERVED_SIZE
    }
}

// ------------------------------------------------------------------
//          Сериализация / десериализация (все → Vec<u8>)
// ------------------------------------------------------------------

fn serialize_user_count(data: &UserCountRaw) -> Vec<u8> {
    let mut out = Vec::with_capacity(12);
    out.extend_from_slice(&FORMAT_DESCRIPTOR.to_le_bytes());
    out.extend_from_slice(&data.count.to_le_bytes());
    out
}

fn deserialize_user_count(buf: &[u8]) -> Result<UserCountRaw> {
    require!(buf.len() >= 12, ErrorCodeNew::InvalidAccountSize);
    let mut descr = [0u8; 4];
    descr.copy_from_slice(&buf[..4]);
    require!(u32::from_le_bytes(descr) == FORMAT_DESCRIPTOR, ErrorCodeNew::InvalidAccountSize);
    let mut cnt = [0u8; 8];
    cnt.copy_from_slice(&buf[4..12]);
    Ok(UserCountRaw { count: u64::from_le_bytes(cnt) })
}

fn serialize_search_index(data: &SearchIndexRaw) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32);
    out.extend_from_slice(&FORMAT_DESCRIPTOR.to_le_bytes());
    out.extend_from_slice(data.big_user.as_ref());
    out
}

fn deserialize_search_index(buf: &[u8]) -> Result<SearchIndexRaw> {
    require!(buf.len() >= 36, ErrorCodeNew::InvalidAccountSize);
    let mut descr = [0u8; 4];
    descr.copy_from_slice(&buf[..4]);
    require!(u32::from_le_bytes(descr) == FORMAT_DESCRIPTOR, ErrorCodeNew::InvalidAccountSize);
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&buf[4..36]);
    Ok(SearchIndexRaw { big_user: Pubkey::new_from_array(pk) })
}

fn serialize_big_user(data: &BigUserData, account_size: usize) -> Vec<u8> {
    let base_len = BigUserData::byte_len();
    let mut out = Vec::with_capacity(account_size);
    out.extend_from_slice(&FORMAT_DESCRIPTOR.to_le_bytes());
    out.extend_from_slice(&data.id.to_le_bytes());
    out.push(data.login_len);
    out.extend_from_slice(&data.login);
    out.extend_from_slice(data.pubkey.as_ref());
    out.extend_from_slice(&data.created_at.to_le_bytes());
    out.extend_from_slice(&data.updated_at.to_le_bytes());
    out.extend_from_slice(&data.reserved);
    // Заполняем оставшееся пространство нулями, если пользователь попросил
    // account_size больше минимального.
    if account_size > base_len {
        out.resize(account_size, 0);
    }
    out
}

// Обратная функция десериализации при необходимости можно добавить позже.

// ------------------------------------------------------------------
//                      Валидационные утилиты
// ------------------------------------------------------------------

fn validate_login(login: &str) -> Result<()> {
    if login.len() > 32 {
        return err!(ErrorCodeNew::InvalidLoginFormat);
    }
    for c in login.chars() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return err!(ErrorCodeNew::InvalidLoginFormat);
        }
    }
    Ok(())
}

fn is_premium_name(login: &str) -> bool {
    // TODO: добавить более сложные проверки «красоты» имени
    login.len() < 8
}

fn validate_pubkey(pk: &Pubkey) -> Result<()> {
    if pk.to_bytes().len() != 32 {
        return err!(ErrorCodeNew::InvalidPubkey);
    }
    Ok(())
}

fn validate_account_size(size: usize) -> Result<()> {
    if size < 200 || size > 4000 || size < BigUserData::byte_len() {
        return err!(ErrorCodeNew::InvalidAccountSize);
    }
    Ok(())
}

// ------------------------------------------------------------------
//            ONE‑TIME инициализация счётчика пользователей
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitSystem<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,


    /// CHECK: Аккаунт создаётся вручную и проверяется по seeds
    #[account(mut, seeds = [USER_COUNT_PDA_SEED], bump)]
    pub user_count: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn init_system(ctx: Context<InitSystem>) -> Result<()> {
    // ----------------------------------------------------------
    // Если PDA уже существует – просто выходим.
    // ----------------------------------------------------------
    if !ctx.accounts.user_count.data_is_empty() {
        return Ok(());
    }

    // ----------------------------------------------------------
    // Создаём PDA через CPI
    // ----------------------------------------------------------
    let rent = Rent::get()?;
    let space = 12; // 4 descr + 8 count
    let lamports = rent.minimum_balance(space);
    let (pda, bump) = Pubkey::find_program_address(&[USER_COUNT_PDA_SEED], ctx.program_id);
    let seeds = &[USER_COUNT_PDA_SEED, &[bump]];

    // Инструкция SystemProgram::CreateAccount
    let ix = system_instruction::create_account(
        &ctx.accounts.admin.key(),
        &pda,
        lamports,
        space as u64,
        ctx.program_id,
    );

    invoke_signed(
        &ix,
        &[
            ctx.accounts.admin.to_account_info(),
            ctx.accounts.user_count.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[seeds],
    )?;

    // ----------------------------------------------------------
    // Записываем стартовое значение счётчика
    // ----------------------------------------------------------
    let raw = serialize_user_count(&UserCountRaw { count: 0 });
    ctx.accounts.user_count.data.borrow_mut()[..raw.len()].copy_from_slice(&raw);

    Ok(())
}

// ------------------------------------------------------------------
//                Расширенная регистрация пользователя
// ------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(login: String, account_size: u32)]
pub struct RegisterUser2<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// Счётчик пользователей
    /// CHECK: Счётчик пользователей читается/пишется вручную как массив байт с дескриптором
    #[account(mut, seeds=[USER_COUNT_PDA_SEED], bump)]
    pub user_count: UncheckedAccount<'info>,

    /// Поисковый PDA по имени (может не существовать)
    /// CHECK: Создаётся вручную, проверяется на соответствие PDA по login
    #[account(mut)]
    pub search_by_name: UncheckedAccount<'info>,

    /// BigUser PDA (может не существовать)
    /// CHECK: Создаётся вручную, проверяется на соответствие PDA по login
    #[account(mut)]
    pub big_user_pda: UncheckedAccount<'info>,

    /// Поисковый PDA по id (будет создан внутри)

    /// CHECK: Создаётся вручную, проверяется на соответствие PDA по id    #[account(mut)]
    pub search_by_id: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn register_user2(
    ctx: Context<RegisterUser2>,
    login: String,
    new_pubkey: Pubkey,
    account_size: u32,
) -> Result<()> {
    // 1. Проверка логина
    validate_login(&login)?;

    // 2. Проверка существования поискового PDA по имени
    let (name_pda, name_bump) = Pubkey::find_program_address(&[SEARCH_NAME_PDA_PREFIX, login.as_bytes()], ctx.program_id);
    require_keys_eq!(ctx.accounts.search_by_name.key(), name_pda, ErrorCodeNew::InvalidAccountSize);
    if !ctx.accounts.search_by_name.data_is_empty() {
        return err!(ErrorCodeNew::UserAlreadyExists);
    }

    // 3. Премиум‑имя
    if is_premium_name(&login) {
        return err!(ErrorCodeNew::PremiumName);
    }

    // 4. Публичный ключ и размер PDA
    validate_pubkey(&new_pubkey)?;
    validate_account_size(account_size as usize)?;

    // 5. Читаем и инкрементируем счётчик
    let mut cnt_data = ctx.accounts.user_count.data.borrow_mut();
    let current_cnt = deserialize_user_count(&cnt_data[..])?.count;
    let new_id = current_cnt + 1;
    let user_count_serialized = serialize_user_count(&UserCountRaw { count: new_id });
    cnt_data[..user_count_serialized.len()].copy_from_slice(&user_count_serialized);

    // 6. Создаём BigUser PDA
    let (big_pda, big_bump) = Pubkey::find_program_address(&[BIG_USER_PDA_PREFIX, login.as_bytes()], ctx.program_id);
    require_keys_eq!(ctx.accounts.big_user_pda.key(), big_pda, ErrorCodeNew::InvalidAccountSize);

    if ctx.accounts.big_user_pda.lamports() == 0 {
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(account_size as usize);
        let ix = system_instruction::create_account(
            &ctx.accounts.signer.key(),
            &big_pda,
            lamports,
            account_size as u64,
            ctx.program_id,
        );
        let seeds = &[BIG_USER_PDA_PREFIX, login.as_bytes(), &[big_bump]];
        invoke_signed(
            &ix,
            &[
                ctx.accounts.signer.to_account_info(),
                ctx.accounts.big_user_pda.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[seeds],
        )?;
    }

    // 7. Пишем данные пользователя
    let clock = Clock::get()?;
    let mut login_bytes = [0u8; 32];
    login_bytes[..login.len()].copy_from_slice(login.as_bytes());

    let big_user_struct = BigUserData {
        id: new_id,
        login_len: login.len() as u8,
        login: login_bytes,
        pubkey: new_pubkey,
        created_at: clock.unix_timestamp,
        updated_at: clock.unix_timestamp,
        reserved: [0u8; RESERVED_SIZE],
    };
    let serialized_big = serialize_big_user(&big_user_struct, account_size as usize);
    ctx.accounts.big_user_pda.data.borrow_mut()[..serialized_big.len()].copy_from_slice(&serialized_big);

    // 8. Создаём поисковый PDA по имени (если ещё не создан)
    if ctx.accounts.search_by_name.lamports() == 0 {
        let rent = Rent::get()?;
        let space = 36; // 4 descr + 32 pubkey
        let lamports = rent.minimum_balance(space);
        let ix = system_instruction::create_account(
            &ctx.accounts.signer.key(),
            &name_pda,
            lamports,
            space as u64,
            ctx.program_id,
        );
        let seeds = &[SEARCH_NAME_PDA_PREFIX, login.as_bytes(), &[name_bump]];
        invoke_signed(
            &ix,
            &[
                ctx.accounts.signer.to_account_info(),
                ctx.accounts.search_by_name.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[seeds],
        )?;
    }
    let search_by_name_raw = serialize_search_index(&SearchIndexRaw { big_user: big_pda });
    ctx.accounts.search_by_name.data.borrow_mut()[..search_by_name_raw.len()].copy_from_slice(&search_by_name_raw);

    // 9. Создаём поисковый PDA по id
    let id_seed = new_id.to_le_bytes();
    let (id_pda, id_bump) = Pubkey::find_program_address(&[SEARCH_ID_PDA_PREFIX, &id_seed], ctx.program_id);
    require_keys_eq!(ctx.accounts.search_by_id.key(), id_pda, ErrorCodeNew::InvalidAccountSize);

    if ctx.accounts.search_by_id.lamports() == 0 {
        let rent = Rent::get()?;
        let space = 36;
        let lamports = rent.minimum_balance(space);
        let ix = system_instruction::create_account(
            &ctx.accounts.signer.key(),
            &id_pda,
            lamports,
            space as u64,
            ctx.program_id,
        );
        let seeds = &[SEARCH_ID_PDA_PREFIX, &id_seed, &[id_bump]];
        invoke_signed(
            &ix,
            &[
                ctx.accounts.signer.to_account_info(),
                ctx.accounts.search_by_id.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[seeds],
        )?;
    }
    let search_by_id_raw = serialize_search_index(&SearchIndexRaw { big_user: big_pda });
    ctx.accounts.search_by_id.data.borrow_mut()[..search_by_id_raw.len()].copy_from_slice(&search_by_id_raw);

    msg!("Пользователь '{}' (id = {}) успешно зарегистрирован", login, new_id);
    Ok(())
}

// ------------------------------------------------------------------
//          Утилиты чтения адреса BigUser PDA off‑chain (raw)
// ------------------------------------------------------------------

pub fn big_user_by_name<'info>(program_id: &Pubkey, name: &str, accounts: &[AccountInfo<'info>]) -> Option<Pubkey> {
    let (pda, _) = Pubkey::find_program_address(&[SEARCH_NAME_PDA_PREFIX, name.as_bytes()], program_id);
    accounts
        .iter()
        .find(|a| a.key == &pda && !a.data_is_empty())
        .and_then(|a| deserialize_search_index(&a.data.borrow()).ok())
        .map(|idx| idx.big_user)
}

pub fn big_user_by_id<'info>(program_id: &Pubkey, id: u64, accounts: &[AccountInfo<'info>]) -> Option<Pubkey> {
    let (pda, _) = Pubkey::find_program_address(&[SEARCH_ID_PDA_PREFIX, &id.to_le_bytes()], program_id);
    accounts
        .iter()
        .find(|a| a.key == &pda && !a.data_is_empty())
        .and_then(|a| deserialize_search_index(&a.data.borrow()).ok())
        .map(|idx| idx.big_user)
}

//--------------------------------------------------------------------
//                 Хелпер для off‑chain: адрес счётчика
//--------------------------------------------------------------------

pub fn user_count_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[USER_COUNT_PDA_SEED], program_id)
}
