//! -----------------------------------------------------------------------------
//! Расширенная регистрация пользователя (register_user2)
//! -----------------------------------------------------------------------------
//! Версия **без** автосериализации Anchor: все данные сохраняются как «сырой» массив
//! байт (manual‑serialization). В начале КАЖДОЙ PDA, кроме счётчика пользователей,
//! теперь находится **4‑байтный дескриптор формата** ( `FORMAT_DESCRIPTOR = 1` ).
//!
//! > Обратите внимание: **Счётчик пользователей** (`UserCountRaw`) БОЛЬШЕ не хранит
//! > дескриптор — в нём теперь только 8‑байтное `u64` (идентификатор последнего
//! > пользователя). Размер PDA счётчика = **8 байт**.
//!
//! -----------------------------------------------------------------------------
//! СОДЕРЖАНИЕ МОДУЛЯ
//! -----------------------------------------------------------------------------
//! 1. Константы и префиксы PDA.
//! 2. Пользовательские ошибки (`ErrorCodeNew`).
//! 3. Структуры данных (in‑memory, для расчётов).
//! 4. Сериализация / десериализация.
//! 5. Валидация логина / ключа / размера PDA.
//! 6. Одноразовая инструкция `init_system` – создаёт счётчик пользователей.
//! 7. Инструкция `register_user2` – расширенная регистрация пользователя.
//! 8. Общая функция `create_pda_if_needed` – создаёт любой PDA при необходимости.
//! 9. Вспомогательные off‑chain‑функции поиска PDA.
//! -----------------------------------------------------------------------------
//! ПОДКЛЮЧЕНИЕ В `lib.rs`
//! -----------------------------------------------------------------------------
//! ```rust
//! mod user_module;                   // подключаем модуль
//! pub use user_module::{             // экспортируем инструкции наружу
//!     init_system,
//!     register_user2,
//! };
//! ```
//! -----------------------------------------------------------------------------

use anchor_lang::prelude::*;                   // re‑export всех полезных типов Anchor
use anchor_lang::solana_program::{             // низкоуровневые инструкции Solana
                                               clock::Clock,                              // доступ к системным часам (unix_timestamp)
                                               program::invoke_signed,                    // CPI‑вызов с PDA‑подписью
                                               system_instruction,                        // встроенные инструкции SystemProgram
};

// -----------------------------------------------------------------------------
//                              КОНСТАНТЫ
// -----------------------------------------------------------------------------

/// Общий дескриптор формата всех «сложных» PDA (BigUser / search‑индексы).
pub const FORMAT_DESCRIPTOR: u32 = 1;          // 4‑байтовое LE‑число ("signature")

/// seed PDA счётчика пользователей (храним **u64** без дескриптора)
pub const USER_COUNT_PDA_SEED: &[u8] = b"user_count"; // bytes‑литерал = seed
/// Префикс PDA «больших» аккаунтов пользователя
pub const BIG_USER_PDA_PREFIX: &[u8] = b"big_user";
/// Префикс поискового PDA по имени
pub const SEARCH_NAME_PDA_PREFIX: &[u8] = b"search_name";
/// Префикс поискового PDA по id
pub const SEARCH_ID_PDA_PREFIX: &[u8] = b"search_id";

/// Размер зарезервированного поля внутри `BigUserData`
const RESERVED_SIZE: usize = 1024;             // можно увеличивать при миграциях

// Минимальный объём пользовательских данных (без пользовательского "extra")
const MIN_BIG_USER_DATA_SIZE: usize =          // compile‑time расчёт
    4  /*descr*/
        + 8  /*id*/
        + 1  /*login_len*/
        + 32 /*login[32]*/
        + 32 /*pubkey*/
        + 8  /*created_at*/
        + 8  /*updated_at*/;

// -----------------------------------------------------------------------------
//                                    ОШИБКИ
// -----------------------------------------------------------------------------

#[error_code]                                  // макрос Anchor для перечисления ошибок
pub enum ErrorCodeNew {
    #[msg("Неверный формат имени пользователя: допускаются только a-z, 0-9 и _ (до 32 символов)")]
    InvalidLoginFormat,                        // ошибка валидации логина

    #[msg("Пользователь с таким именем уже существует")]
    UserAlreadyExists,                         // имя занято

    #[msg("Имя является платным премиум-именем – требуется отдельная оплата")]
    PremiumName,                               // короткие имена = premium

    #[msg("Неверный публичный ключ (должен состоять из 32 байт)")]
    InvalidPubkey,                             // pubkey ≠ 32 bytes

    #[msg("Неверный или неподдерживаемый размер PDA (должно быть 200‑4000 байт)")]
    InvalidAccountSize,                        // account_size вне допустимого диапазона
}

// -----------------------------------------------------------------------------
//                        СТРУКТУРЫ (in‑memory, RAM)
// -----------------------------------------------------------------------------

/// Счётчик пользователей (**теперь содержит ТОЛЬКО u64**, без дескриптора)
pub struct UserCountRaw {
    pub count: u64,                            // идентификатор последнего созданного пользователя
}

/// Поисковый PDA (индекс) → хранит адрес `BigUser` PDA
pub struct SearchIndexRaw {
    pub big_user: Pubkey,                      // куда указывает индекс
}

/// Полные данные пользователя (PDA «BigUser»)
pub struct BigUserData {
    pub id: u64,                               // уникальный numeric id
    pub login_len: u8,                         // фактическая длина логина
    pub login: [u8; 32],                       // логин ASCII, padded нулями
    pub pubkey: Pubkey,                        // публичный ключ пользователя
    pub created_at: i64,                       // unix‑timestamp создания
    pub updated_at: i64,                       // unix‑timestamp последнего обновления
    pub reserved: [u8; RESERVED_SIZE],         // резерв на будущее
}

impl BigUserData {
    /// Минимальная длина сериализованного блока данных (без user‑extra).   
    pub const fn byte_len() -> usize {         // const‑fn = вычисление на этапе компиляции
        4 /*descr*/
            + 8 + 1 + 32 + 32 + 8 + 8
            + RESERVED_SIZE
    }
}

// -----------------------------------------------------------------------------
//                СЕРИАЛИЗАЦИЯ / ДЕСЕРИАЛИЗАЦИЯ (RAM ↔ Vec<u8>)
// -----------------------------------------------------------------------------

// ------------------------------- UserCount -----------------------------------

/// Сериализуем `UserCountRaw` → вектор из 8 байт (LE‑u64)
fn serialize_user_count(data: &UserCountRaw) -> Vec<u8> {
    data.count.to_le_bytes().to_vec()          // просто LE‑u64 → Vec<u8>
}

/// Десериализуем массив байт в `UserCountRaw`
fn deserialize_user_count(buf: &[u8]) -> Result<UserCountRaw> {
    require!(buf.len() >= 8, ErrorCodeNew::InvalidAccountSize); // надо минимум 8 байт
    let mut cnt = [0u8; 8];                     // временный массив для копирования
    cnt.copy_from_slice(&buf[..8]);             // копируем первые 8 байт
    Ok(UserCountRaw { count: u64::from_le_bytes(cnt) }) // превращаем в u64
}

// ------------------------------ SearchIndex ----------------------------------

/// Сериализация поискового индекса (descr + pubkey)
fn serialize_search_index(data: &SearchIndexRaw) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32);   // зарезервировали память
    out.extend_from_slice(&FORMAT_DESCRIPTOR.to_le_bytes()); // 4‑байтный descr
    out.extend_from_slice(data.big_user.as_ref());           // 32‑байтный pubkey
    out
}

/// Десериализация поискового индекса (Vec<u8> → struct)
fn deserialize_search_index(buf: &[u8]) -> Result<SearchIndexRaw> {
    require!(buf.len() >= 36, ErrorCodeNew::InvalidAccountSize); // 4 + 32 = 36
    // проверяем дескриптор
    let mut descr = [0u8; 4];
    descr.copy_from_slice(&buf[..4]);
    require!(u32::from_le_bytes(descr) == FORMAT_DESCRIPTOR, ErrorCodeNew::InvalidAccountSize);
    // копируем pubkey
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&buf[4..36]);
    Ok(SearchIndexRaw { big_user: Pubkey::new_from_array(pk) })
}

// ------------------------------- BigUser -------------------------------------

/// Сериализация `BigUserData` с учётом желаемого `account_size`
fn serialize_big_user(data: &BigUserData, account_size: usize) -> Vec<u8> {
    let base_len = BigUserData::byte_len();     // «реальный» минимум
    let mut out = Vec::with_capacity(account_size); // резервируем весь объём
    // последовательная запись полей
    out.extend_from_slice(&FORMAT_DESCRIPTOR.to_le_bytes());   // 4‑байтный descr
    out.extend_from_slice(&data.id.to_le_bytes());             // id (u64 LE)
    out.push(data.login_len);                                  // длина логина (u8)
    out.extend_from_slice(&data.login);                        // сам логин (32)
    out.extend_from_slice(data.pubkey.as_ref());               // pubkey (32)
    out.extend_from_slice(&data.created_at.to_le_bytes());     // created_at (i64 LE)
    out.extend_from_slice(&data.updated_at.to_le_bytes());     // updated_at (i64 LE)
    out.extend_from_slice(&data.reserved);                     // reserved bytes
    if account_size > base_len {                               // если пользователь запросил > base_len
        out.resize(account_size, 0);                           // добиваем нулями до нужного объёма
    }
    out
}

// (Обратную десериализацию можно добавить позднее при необходимости)

// -----------------------------------------------------------------------------
//                       ВАЛИДАЦИОННЫЕ УТИЛИТЫ
// -----------------------------------------------------------------------------

/// Проверка логина на формат (a‑z, 0‑9, "_", ≤ 32)
fn validate_login(login: &str) -> Result<()> {
    if login.len() > 32 {                       // длина > 32 → ошибка
        return err!(ErrorCodeNew::InvalidLoginFormat);
    }
    for c in login.chars() {                    // проходим по каждому символу
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return err!(ErrorCodeNew::InvalidLoginFormat);
        }
    }
    Ok(())                                      // всё ок
}

/// Простая проверка «короткое имя = премиум» (можно усложнить)
fn is_premium_name(login: &str) -> bool {
    login.len() < 8                             // примитив: < 8 символов → платное
}

/// Проверка публичного ключа (длина всегда 32 байта)
fn validate_pubkey(pk: &Pubkey) -> Result<()> {
    if pk.to_bytes().len() != 32 {
        return err!(ErrorCodeNew::InvalidPubkey);
    }
    Ok(())
}

/// Проверка желаемого `account_size` (≥ 200, ≤ 4000, ≥ минимального big_user)
fn validate_account_size(size: usize) -> Result<()> {
    if size < 200 || size > 4000 || size < BigUserData::byte_len() {
        return err!(ErrorCodeNew::InvalidAccountSize);
    }
    Ok(())
}

// -----------------------------------------------------------------------------
//          ОБЩАЯ ФУНКЦИЯ СОЗДАНИЯ PDA, ЕСЛИ ЕГО ЕЩЁ НЕТ
// -----------------------------------------------------------------------------

/// Создаёт PDA‑аккаунт, если на нём ещё **0 лампортов** (то есть счёт не создан).
/// Все повторяющиеся действия (расчёт ренты, вызов SystemProgram::CreateAccount,
/// подпись через seeds) объединены в одну функцию.
fn create_pda_if_needed<'info>(
    payer: &Signer<'info>,                      // аккаунт‑плательщик (обычно tx‑signer)
    pda_account: &UncheckedAccount<'info>,      // AccountInfo PDA (может быть пустым)
    pda_key: &Pubkey,                           // ожидаемый адрес PDA
    seeds: &[&[u8]],                            // seeds + bump, которые формируют PDA
    space: usize,                               // сколько байт необходимо хранить
    program_id: &Pubkey,                        // адрес текущей программы
    system_program: &Program<'info, System>,    // встроенная системная программа
) -> Result<()> {
    if pda_account.lamports() > 0 {             // уже создан → ничего не делаем
        return Ok(());
    }

    let rent = Rent::get()?;                    // получаем параметры аренды
    let lamports = rent.minimum_balance(space); // минимум для rent‑exempt

    // формируем инструкцию `SystemProgram::CreateAccount`
    let ix = system_instruction::create_account(
        &payer.key(),                           // кто платит
        pda_key,                                // какой PDA создаём
        lamports,                               // сколько лампортов перечислить
        space as u64,                           // объём в байтах
        program_id,                             // владелец аккаунта – наша программа
    );

    // вызываем инструкцию через CPI + PDA‑подпись
    invoke_signed(
        &ix,
        &[
            payer.to_account_info(),            // плательщик
            pda_account.to_account_info(),      // создаваемый PDA
            system_program.to_account_info(),   // системная программа
        ],
        &[seeds],                               // seeds для подписи PDA
    )?;                                          // "?" = propagate ошибка, если есть

    Ok(())
}

// -----------------------------------------------------------------------------
//          ИНИЦИАЛИЗАЦИЯ СИСТЕМЫ (одноразово) – счётчик пользователей
// -----------------------------------------------------------------------------

#[derive(Accounts)]                             // макрос Anchor: описание контекста
pub struct InitSystem<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,                  // кто инициализирует (платит за PDA)

    /// CHECK: PDA создаётся вручную → безопасно помечаем Unchecked
    #[account(mut)]
    pub user_count: UncheckedAccount<'info>,    // счётчик пользователей

    pub system_program: Program<'info, System>, // встроенная системная программа
}

/// Инструкция `init_system` – вызывается **один раз** для создания PDA счётчика.

/// Инструкция `init_system` – вызывается **один раз** для создания PDA счётчика.
pub fn init_system(ctx: Context<InitSystem>) -> Result<()> {
    // ----------------------------------------------------------------------
    // 0. Вычисляем правильный адрес PDA user_count
    // ----------------------------------------------------------------------
    let (expected_pda, bump) = Pubkey::find_program_address(&[USER_COUNT_PDA_SEED], ctx.program_id);

    // Сверяем с переданным аккаунтом — защита от подмены
    require_keys_eq!(
        ctx.accounts.user_count.key(),
        expected_pda,
        ErrorCodeNew::InvalidAccountSize // Можно завести свою ошибку типа InvalidUserCountPda
    );

    let seeds: &[&[u8]] = &[USER_COUNT_PDA_SEED, &[bump]];
    let space = 8usize;

    // ----------------------------------------------------------------------
    // 1. Если PDA уже существует → просто выходим
    // ----------------------------------------------------------------------
    if !ctx.accounts.user_count.data_is_empty() {
        return Ok(());
    }

    // ----------------------------------------------------------------------
    // 2. Создаём PDA счётчика (если он всё ещё пустой)
    // ----------------------------------------------------------------------
    create_pda_if_needed(
        &ctx.accounts.admin,
        &ctx.accounts.user_count,
        &expected_pda,
        seeds,
        space,
        ctx.program_id,
        &ctx.accounts.system_program,
    )?;

    // ----------------------------------------------------------------------
    // 3. Записываем начальное значение счётчика (0)
    // ----------------------------------------------------------------------
    let raw = serialize_user_count(&UserCountRaw { count: 0 });
    ctx.accounts.user_count.data.borrow_mut()[..raw.len()]
        .copy_from_slice(&raw);

    Ok(())
}

/*pub fn init_system(ctx: Context<InitSystem>) -> Result<()> {
    // 1. Если PDA уже существует → просто выходим (ничего не делаем)
    if !ctx.accounts.user_count.data_is_empty() { // data_is_empty = false → PDA уже создан
        return Ok(());
    }

    // 2. Создаём PDA счётчика (space = 8 байт, без дескриптора)
    let space = 8usize;                         // 8 байт → одно поле u64
    let (pda_key, bump) = Pubkey::find_program_address( // получаем PDA‑адрес + bump
                                                        &[USER_COUNT_PDA_SEED],                 // seeds
                                                        ctx.program_id,                         // адрес программы
    );

    let seeds: &[&[u8]] = &[USER_COUNT_PDA_SEED, &[bump]]; // seeds массив для подписи

    create_pda_if_needed(                       // создаём PDA, если нужно
                                                &ctx.accounts.admin,                    // кто платит
                                                &ctx.accounts.user_count,               // PDA‑аккаунт
                                                &pda_key,                               // ключ PDA
                                                seeds,                                  // seeds + bump
                                                space,                                  // 8 байт
                                                ctx.program_id,                         // id программы
                                                &ctx.accounts.system_program,           // системная программа
    )?;

    // 3. Записываем начальное значение счётчика (0)
    let raw = serialize_user_count(&UserCountRaw { count: 0 }); // Vec<u8> из 8 байт
    ctx.accounts.user_count.data.borrow_mut()[..raw.len()].copy_from_slice(&raw); // пишем в PDA

    Ok(())                                       // done
}
*/
// -----------------------------------------------------------------------------
//                 ИНСТРУКЦИЯ РЕГИСТРАЦИИ ПОЛЬЗОВАТЕЛЯ
// -----------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(login: String, account_size: u32)] // прокидываем параметры для проверки seeds
pub struct RegisterUser2<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,                  // пользователь, вызывающий регистрацию

    /// CHECK: PDA счётчика (читаем + пишем вручную как сырой массив)
    #[account(mut)]
    pub user_count: UncheckedAccount<'info>,

    /// CHECK: поисковый PDA по имени (может быть пустым)
    #[account(mut)]
    pub search_by_name: UncheckedAccount<'info>,

    /// CHECK: большой PDA пользователя (может быть пустым)
    #[account(mut)]
    pub big_user_pda: UncheckedAccount<'info>,

    /// CHECK: поисковый PDA по id (может быть пустым, создаём внутри)
    #[account(mut)]
    pub search_by_id: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Основная инструкция: регистрирует нового пользователя.
pub fn register_user2(
    ctx: Context<RegisterUser2>,                // контекст (счета + системные)
    login: String,                              // желаемый логин
    new_pubkey: Pubkey,                         // публичный ключ пользователя
    account_size: u32,                          // желаемый размер BigUser PDA
) -> Result<()> {
    // ----------------------------------------------------------------------
    // 1. Проверяем логин на корректность
    // ----------------------------------------------------------------------
    validate_login(&login)?;                    // формат + длина

    // ----------------------------------------------------------------------
    // 2. Проверяем, существует ли уже индекс по этому логину
    // ----------------------------------------------------------------------
    let (name_pda_key, name_bump) = Pubkey::find_program_address(
        &[SEARCH_NAME_PDA_PREFIX, login.as_bytes()],
        ctx.program_id,
    );
    if !ctx.accounts.search_by_name.data_is_empty() {               // данные уже есть → пользователь существует
        return err!(ErrorCodeNew::UserAlreadyExists);
    }

    // ----------------------------------------------------------------------
    // 3. Проверяем, не премиум‑ли имя (короткие имена платные)
    // ----------------------------------------------------------------------
    if is_premium_name(&login) {
        return err!(ErrorCodeNew::PremiumName);
    }

    // ----------------------------------------------------------------------
    // 4. Проверяем публичный ключ и желаемый размер PDA
    // ----------------------------------------------------------------------
    validate_pubkey(&new_pubkey)?;              // pubkey = 32 bytes
    validate_account_size(account_size as usize)?; // размер PDA в рамках правила

    // ----------------------------------------------------------------------
    // 5. Загружаем PDA счётчика пользователей — по адресу, вычисленному внутри
    // ----------------------------------------------------------------------

    // 1. Вычисляем ожидаемый PDA по сидам
    let (user_count_pda_key, _) =
        Pubkey::find_program_address(&[USER_COUNT_PDA_SEED], ctx.program_id);

    // 2. Ищем этот аккаунт среди remaining_accounts (не в ctx.accounts!)
    let user_count_account = ctx.remaining_accounts
        .iter()
        .find(|acc| acc.key == &user_count_pda_key)
        .ok_or_else(|| error!(ErrorCodeNew::InvalidAccountSize))?;

    // 3. Загружаем данные из PDA, десериализуем счётчик
    let mut user_count_data = user_count_account.data.borrow_mut();
    let current_cnt = deserialize_user_count(&user_count_data[..])?.count;

    // 4. Увеличиваем счётчик на 1 и сериализуем обратно
    let new_id = current_cnt + 1;
    let serialized = serialize_user_count(&UserCountRaw { count: new_id });
    user_count_data[..serialized.len()].copy_from_slice(&serialized);
    drop(user_count_data);
                           // явный drop borrow (необязательно, но наглядно)
    
    
    // ----------------------------------------------------------------------
    // 6. Создаём (при необходимости) BigUser PDA
    // ----------------------------------------------------------------------
    let (big_pda_key, big_bump) = Pubkey::find_program_address(
        &[BIG_USER_PDA_PREFIX, login.as_bytes()],
        ctx.program_id,
    );

    let big_seeds: &[&[u8]] = &[BIG_USER_PDA_PREFIX, login.as_bytes(), &[big_bump]]; // seeds + bump
    create_pda_if_needed(
        &ctx.accounts.signer,                   // payer
        &ctx.accounts.big_user_pda,             // PDA‑аккаунт
        &big_pda_key,                           // ключ PDA
        big_seeds,                              // seeds array
        account_size as usize,                  // space
        ctx.program_id,                         // id программы
        &ctx.accounts.system_program,           // системная программа
    )?;

    // ----------------------------------------------------------------------
    // 7. Записываем данные пользователя в BigUser PDA
    // ----------------------------------------------------------------------
    let clock = Clock::get()?;                  // берём текущий unix_timestamp

    let mut login_bytes = [0u8; 32];            // zero‑padded массив под логин
    login_bytes[..login.len()].copy_from_slice(login.as_bytes()); // копируем логин

    // формируем структуру BigUser
    let big_user_struct = BigUserData {
        id: new_id,
        login_len: login.len() as u8,
        login: login_bytes,
        pubkey: new_pubkey,
        created_at: clock.unix_timestamp,
        updated_at: clock.unix_timestamp,
        reserved: [0u8; RESERVED_SIZE],
    };

    // сериализуем структуру в Vec<u8>
    let serialized_big = serialize_big_user(&big_user_struct, account_size as usize);
    // пишем bytes в PDA
    ctx.accounts.big_user_pda.data.borrow_mut()[..serialized_big.len()].copy_from_slice(&serialized_big);

    // ----------------------------------------------------------------------
    // 8. Создаём / обновляем поисковый PDA по имени
    // ----------------------------------------------------------------------
    let name_seeds: &[&[u8]] = &[SEARCH_NAME_PDA_PREFIX, login.as_bytes(), &[name_bump]];
    create_pda_if_needed(
        &ctx.accounts.signer,
        &ctx.accounts.search_by_name,
        &name_pda_key,
        name_seeds,
        36,                                    // 4 descr + 32 pubkey
        ctx.program_id,
        &ctx.accounts.system_program,
    )?;

    // записываем индекс (descr + pubkey)
    let search_by_name_raw = serialize_search_index(&SearchIndexRaw { big_user: big_pda_key });
    ctx.accounts.search_by_name.data.borrow_mut()[..search_by_name_raw.len()].copy_from_slice(&search_by_name_raw);

    // ----------------------------------------------------------------------
    // 9. Создаём / обновляем поисковый PDA по id
    // ----------------------------------------------------------------------
    let id_seed = new_id.to_le_bytes();
    let (id_pda_key, id_bump) = Pubkey::find_program_address(&[SEARCH_ID_PDA_PREFIX, &id_seed], ctx.program_id);

    let id_seeds: &[&[u8]] = &[SEARCH_ID_PDA_PREFIX, &id_seed, &[id_bump]];
    create_pda_if_needed(
        &ctx.accounts.signer,
        &ctx.accounts.search_by_id,
        &id_pda_key,
        id_seeds,
        36,                                    // 4 descr + 32 pubkey
        ctx.program_id,
        &ctx.accounts.system_program,
    )?;

    let search_by_id_raw = serialize_search_index(&SearchIndexRaw { big_user: big_pda_key });
    ctx.accounts.search_by_id.data.borrow_mut()[..search_by_id_raw.len()].copy_from_slice(&search_by_id_raw);

    // ----------------------------------------------------------------------
    // 10. Логируем успешную регистрацию
    // ----------------------------------------------------------------------
    msg!(
        "Пользователь '{}' зарегистрирован (id = {}, pubkey = {})",
        login, new_id, new_pubkey
    );
    Ok(())
}

// -----------------------------------------------------------------------------
//              OFF‑CHAIN ХЕЛПЕРЫ ДЛЯ ПОИСКА BigUser PDA
// -----------------------------------------------------------------------------

/// off‑chain‑функция: получить адрес BigUser по имени (если индекс загружен в `accounts`)
pub fn big_user_by_name<'info>(
    program_id: &Pubkey,                       // адрес программы
    name: &str,                                // логин ascii
    accounts: &[AccountInfo<'info>],           // все доступные PDA‑аккаунты
) -> Option<Pubkey> {
    let (pda, _) = Pubkey::find_program_address(&[SEARCH_NAME_PDA_PREFIX, name.as_bytes()], program_id);
    accounts
        .iter()
        .find(|a| a.key == &pda && !a.data_is_empty())
        .and_then(|a| deserialize_search_index(&a.data.borrow()).ok())
        .map(|idx| idx.big_user)
}

/// off‑chain‑функция: получить адрес BigUser по numeric id
pub fn big_user_by_id<'info>(
    program_id: &Pubkey,
    id: u64,
    accounts: &[AccountInfo<'info>],
) -> Option<Pubkey> {
    let (pda, _) = Pubkey::find_program_address(&[SEARCH_ID_PDA_PREFIX, &id.to_le_bytes()], program_id);
    accounts
        .iter()
        .find(|a| a.key == &pda && !a.data_is_empty())
        .and_then(|a| deserialize_search_index(&a.data.borrow()).ok())
        .map(|idx| idx.big_user)
}

// -----------------------------------------------------------------------------
//              ХЕЛПЕР: получить адрес PDA счётчика пользователей
// -----------------------------------------------------------------------------

pub fn user_count_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[USER_COUNT_PDA_SEED], program_id)
}

// -----------------------------------------------------------------------------
//                            КОНЕЦ МОДУЛЯ
// -----------------------------------------------------------------------------
