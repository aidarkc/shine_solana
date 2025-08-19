use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    program::invoke,
    // program::invoke_signed,
    system_instruction,
};
use std::str::FromStr;
use common::utils::{create_and_write_pda, create_pda, safe_read_pda, write_to_pda};
use common::utils::ErrCode;


// Префикс для PDA пользователей по логину
const USER_SEED_PREFIX: &str = "u=";
// Постоянный адрес получателя комиссии    key3
pub const REGISTRATION_FEE_RECEIVER: &str = "6bFc5Gz5qF172GQhK5HpDbWs8F6qcSxdHn5XqAstf1fY";






/// ───────────────────────────────────────────────────────────────────────
///  Структура UserByLogin
/// ───────────────────────────────────────────────────────────────────────
///
/// Содержит:
/// - login: String               — строка (до 255 байт, храним длину + содержимое)
/// - id: u64                     — 8 байт (целое число)
/// - pubkey: Pubkey             — 32 байта
/// - status: u32                — 4 байта
///
/// Формат сериализованных данных:
/// [0..4]      = format_type: u32 (всегда 1)
/// [4..5]      = длина логина: u8
/// [5..(5+len)] = логин
/// [...]       = id: u64
/// [...]       = pubkey: [u8; 32]
/// [...]       = status: u32
/// Всего: 4 + 1 + логин + 8 + 32 + 4 байта
/// ------------------------------------------------------------------------

pub struct UserByLogin {
    pub login: String,    // логин (строка)
    pub id: u64,          // числовой ID
    pub pubkey: Pubkey,   // публичный ключ
    pub status: u32,      // статус
}

/// ───────────────────────────────────────────────────────────────────────
/// 🔧 Сериализация serialize_user_by_login()
/// ───────────────────────────────────────────────────────────────────────
///
/// Сериализует `UserByLogin` в `Vec<u8>`, начиная с format_type = 1
pub fn serialize_user_by_login(user: &UserByLogin) -> Vec<u8> {
    let mut result = Vec::new();

    // ───────────────────────────────
    // 1. format_type (4 байта)
    // ───────────────────────────────
    result.extend_from_slice(&1u32.to_le_bytes()); // формат 1

    // ───────────────────────────────
    // 2. login: длина (u8) + байты
    // ───────────────────────────────
    let login_bytes = user.login.as_bytes();
    let login_len = login_bytes.len();
    let login_len_u8 = login_len.min(255) as u8; // максимум 255 байт

    result.push(login_len_u8); // длина
    result.extend_from_slice(&login_bytes[..login_len_u8 as usize]);

    // ───────────────────────────────
    // 3. id (u64)
    // ───────────────────────────────
    result.extend_from_slice(&user.id.to_le_bytes());

    // ───────────────────────────────
    // 4. pubkey (32 байта)
    // ───────────────────────────────
    result.extend_from_slice(user.pubkey.as_ref());

    // ───────────────────────────────
    // 5. status (4 байта)
    // ───────────────────────────────
    result.extend_from_slice(&user.status.to_le_bytes());

    result
}

/// ───────────────────────────────────────────────────────────────────────
///🔄 Десериализация deserialize_user_by_login()
/// ───────────────────────────────────────────────────────────────────────
///
/// Определяет формат и вызывает соответствующую реализацию
pub fn deserialize_user_by_login(data: &[u8]) -> Result<UserByLogin> {
    // Проверка длины
    if data.len() < 4 {
        return Err(error!(ErrCode::DeserializationError));
    }

    // Считываем format_type
    let format_type = u32::from_le_bytes(data[0..4].try_into().unwrap());

    match format_type {
        1 => deserialize_user_by_login_format1(data),
        _ => Err(error!(ErrCode::UnsupportedFormat)),
    }
}

/// ───────────────────────────────────────────────────────────────────────
/// Распаковываем user_by_login формат 1:
/// ───────────────────────────────────────────────────────────────────────
fn deserialize_user_by_login_format1(data: &[u8]) -> Result<UserByLogin> {
    let mut offset = 4; // пропускаем format_type

    // 1. login (длина + строка)
    let login_len = data.get(offset).ok_or(ErrCode::DeserializationError)? as &u8;
    offset += 1;

    let login_end = offset + (*login_len as usize);
    if data.len() < login_end {
        return Err(error!(ErrCode::DeserializationError));
    }

    let login = std::str::from_utf8(&data[offset..login_end])
        .map_err(|_| error!(ErrCode::DeserializationError))?
        .to_string();
    offset = login_end;

    // 2. id (u64)
    if data.len() < offset + 8 {
        return Err(error!(ErrCode::DeserializationError));
    }
    let id = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
    offset += 8;

    // 3. pubkey (32 байта)
    if data.len() < offset + 32 {
        return Err(error!(ErrCode::DeserializationError));
    }
    let pubkey = Pubkey::new_from_array(data[offset..offset + 32].try_into().unwrap());
    offset += 32;

    // 4. status (u32)
    if data.len() < offset + 4 {
        return Err(error!(ErrCode::DeserializationError));
    }
    let status = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());

    Ok(UserByLogin {
        login,
        id,
        pubkey,
        status,
    })
}




/// ───────────────────────────────────────────────────────────────────────
/// ───────────────────────────────────────────────────────────────────────
/// РАБОТА С user_counter_pda
/// ───────────────────────────────────────────────────────────────────────
/// ───────────────────────────────────────────────────────────────────────
///


/// Константа для сидов PDA-счётчика пользователей
pub const USER_COUNTER_SEED: &str = "user_counter";


/// ───────────────────────────────────────────────────────────────────────
/// Чтение значения счётчика пользователей из PDA
/// ───────────────────────────────────────────────────────────────────────
///
pub fn read_user_counter_pda<'info>(
    counter_pda: &AccountInfo<'info>, // переданный аккаунт
    program_id: &Pubkey,              // ID текущей программы
) -> Result<u64> {
    // Проверяем, что переданный PDA соответствует сиду
    let seeds: &[&[u8]] = &[USER_COUNTER_SEED.as_bytes()];
    let (expected_pda, _) = Pubkey::find_program_address(seeds, program_id);
    require!(counter_pda.key == &expected_pda, ErrCode::InvalidPdaAddress);

    // Безопасное чтение данных
    let raw = safe_read_pda(counter_pda);
    if raw.len() != 8 {
        return Err(error!(ErrCode::EmptyPdaData)); // неверный размер
    }

    // Преобразуем 8 байт в u64
    let value = u64::from_le_bytes(raw.try_into().map_err(|_| ErrCode::DeserializationError)?);
    Ok(value)
}

/// ───────────────────────────────────────────────────────────────────────
/// Запись нового значения счётчика в PDA
/// ───────────────────────────────────────────────────────────────────────
pub fn write_user_counter_pda<'info>(
    counter_pda: &AccountInfo<'info>,
    program_id: &Pubkey,
    value: u64,
) -> Result<()> {
    // Проверяем адрес PDA
    let seeds: &[&[u8]] = &[USER_COUNTER_SEED.as_bytes()];
    let (expected_pda, _) = Pubkey::find_program_address(seeds, program_id);
    require!(counter_pda.key == &expected_pda, ErrCode::InvalidPdaAddress);

    // Сериализуем u64 в 8 байт
    let bytes = value.to_le_bytes().to_vec();

    // Записываем в PDA
    write_to_pda(counter_pda, &bytes)
}

/// ───────────────────────────────────────────────────────────────────────
/// Инициализация PDA счётчика пользователей (однократная)
/// ───────────────────────────────────────────────────────────────────────
///
/// структура вызова
#[derive(Accounts)]
pub struct InitUserCounter<'info> {
    /// Тот, кто платит за создание PDA
    /// CHECK: Это просто подписант, проверяется Anchor через #[account(signer)]
    #[account(mut, signer)]
    pub signer: AccountInfo<'info>,

    /// Аккаунт-счётчик пользователей, должен быть PDA с сидом ["user_counter"]
    /// CHECK: Это PDA, валидность которого проверяется в коде вручную по сид-значению
    #[account(mut)]
    pub counter_pda: AccountInfo<'info>,

    /// Системная программа Solana
    pub system_program: Program<'info, System>,
}
/// и функция
pub fn initialize_user_counter<'info>(
    counter_pda: &AccountInfo<'info>,
    signer: &AccountInfo<'info>,         // платит за создание
    system_program: &AccountInfo<'info>, // системная программа
    program_id: &Pubkey,
) -> Result<()> {
    // Генерация PDA из сидов
    let seeds: &[&[u8]] = &[USER_COUNTER_SEED.as_bytes()];
    let (expected_pda, bump) = Pubkey::find_program_address(seeds, program_id);
    require!(counter_pda.key == &expected_pda, ErrCode::InvalidPdaAddress);

    // Проверка — если PDA уже существует, завершаем с ошибкой
    if counter_pda.owner != &Pubkey::default() {
        msg!("PDA Со счётчиком пользователей уже существует. Система уже инициализированна!");
        return Err(error!(ErrCode::SystemAlreadyInitialized));
    }

    // Полные сиды
    let full_seeds: &[&[u8]] = &[USER_COUNTER_SEED.as_bytes(), &[bump]];

    // Создаём PDA и записываем туда 0
    create_and_write_pda(
        counter_pda,
        signer,
        system_program,
        program_id,
        full_seeds,
        0u64.to_le_bytes().to_vec(), // записываем 0
        8,                           // размер — 8 байт (u64)
    )?;
    msg!("PDA Со счётчиком пользователей успешно создан");
    Ok(())
}





















/// ───────────────────────────────────────────────────────────────────────
/// РЕГИСТРАЦИЯ пользователя (шаг ПЕРВЫЙ) по логину
/// ───────────────────────────────────────────────────────────────────────


pub fn register_user_step_one(
    ctx: Context<RegisterUserStepOne>,
    login: String,
    user_pubkey: Pubkey,
) -> Result<()> {
    // ───────────────────────────────────────────────
    // 1. Проверка валидности логина
    validate_login(&login)?; // вызывает функцию ниже

    // ───────────────────────────────────────────────
    // 2. Проверяем, что логин не является "особым" (зарезервированным)
    let reserved_logins = ["admin", "support", "solana"]; // можно расширить
    require!(
        !reserved_logins.contains(&login.as_str()),
        ErrCode::InvalidLogin
    );

    // ───────────────────────────────────────────────
    // 3. Проверка PDA
    let seed_string = format!("{}{}", USER_SEED_PREFIX, login);
    let seed_bytes = seed_string.as_bytes();
    let (expected_pda, bump) = Pubkey::find_program_address(&[seed_bytes], ctx.program_id);
    require!(
        &expected_pda == ctx.accounts.user_by_login_pda.key,
        ErrCode::InvalidPdaAddress
    );

    // ───────────────────────────────────────────────
    // 4. Проверяем, что PDA ещё не инициализирован
    if ctx.accounts.user_by_login_pda.owner != &Pubkey::default() {
        return Err(error!(ErrCode::UserAlreadyExists));
    }

    // ───────────────────────────────────────────────
    // 5. Перевод 0.01 SOL комиссии за регистрацию
    let expected_receiver = Pubkey::from_str(REGISTRATION_FEE_RECEIVER)
        .map_err(|_| error!(ErrCode::InvalidLogin))?;
    require!(
        ctx.accounts.fee_receiver.key == &expected_receiver,
        ErrCode::InvalidPdaAddress
    );

    let transfer_instruction = system_instruction::transfer(
        ctx.accounts.signer.key,
        ctx.accounts.fee_receiver.key,
        10_000_000, // 0.01 SOL в лампортах
    );
    invoke(
        &transfer_instruction,
        &[
            ctx.accounts.signer.clone(),
            ctx.accounts.fee_receiver.clone(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;

    // ───────────────────────────────────────────────
    // 6. Получаем текущий счётчик
    let current_id = read_user_counter_pda(&ctx.accounts.user_counter, ctx.program_id)?;

    // ───────────────────────────────────────────────
    // 7. Создаём структуру UserByLogin
    let user = UserByLogin {
        login: login.clone(),
        id: current_id + 1,
        pubkey: user_pubkey,
        status: 0,
    };

    let serialized_user = serialize_user_by_login(&user);

    // ───────────────────────────────────────────────
    // 8. Создаём PDA и записываем в него сериализованные данные

    let full_seeds: &[&[u8]] = &[seed_bytes, &[bump]];
    create_pda(
        &ctx.accounts.user_by_login_pda,
        &ctx.accounts.signer,
        &ctx.accounts.system_program.to_account_info(),
        ctx.program_id,
        full_seeds,
        serialized_user.len() as u64,
    )?;

    write_to_pda(&ctx.accounts.user_by_login_pda, &serialized_user)?;

    // ───────────────────────────────────────────────
    // 9. Обновляем счётчик пользователей
    write_user_counter_pda(
        &ctx.accounts.user_counter,
        ctx.program_id,
        current_id + 1,
    )?;

    msg!("✅ Пользователь успешно зарегистрирован: {}", login);
    Ok(())
}


/// Структура аккаунтов для регистрации нового пользователя
#[derive(Accounts)]
pub struct RegisterUserStepOne<'info> {
    /// CHECK: Это просто подписант, валидируется Anchor по ключу и подписи
    /// Подписант — новый пользователь, он платит за создание PDA
    #[account(mut, signer)]
    pub signer: AccountInfo<'info>,

    /// CHECK: это PDA, проверяется вручную через сиды и ключ
    /// PDA счётчика пользователей
    #[account(mut)]
    pub user_counter: AccountInfo<'info>,

    /// CHECK: PDA-аккаунт пользователя, проверяется вручную через сид `"u=" + login`
    /// Новый PDA-аккаунт пользователя по логину
    #[account(mut)]
    pub user_by_login_pda: AccountInfo<'info>,

    /// Системная программа
    pub system_program: Program<'info, System>,

    /// Аккаунт получателя комиссии (проверяется по адресу)
    /// CHECK: проверяется вручную по адресу
    #[account(mut)]
    pub fee_receiver: AccountInfo<'info>,
}

/// Проверяет, что логин состоит из латинских строчных букв, цифр и "_"
/// и длина не превышает 30 символов
pub fn validate_login(login: &str) -> Result<()> {
    if login.len() > 30 {
        return Err(error!(ErrCode::InvalidLogin));
    }

    for ch in login.chars() {
        if !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_') {
            return Err(error!(ErrCode::InvalidLogin));
        }
    }

    Ok(())
}
































/// ───────────────────────────────────────────────────────────────────────
/// Структуры и сериализация UserById
/// ───────────────────────────────────────────────────────────────────────



/// Константа для версии формата сериализации UserById
pub const USER_BY_ID_FORMAT_V1: u32 = 1;




/// Структура, описывающая одно устройство пользователя.
///
/// Содержит:
/// - device_type: тип устройства (1 байт, например: 1 = телефон, 2 = ПК)
/// - device_pubkey: подпись устройства (Pubkey, 32 байта)
/// - x25519_pubkey: публичный ключ X25519 для шифрования (32 байта)
pub struct DeviceInfo {
    pub device_type: u8,
    pub device_pubkey: Pubkey,
    pub x25519_pubkey: Pubkey,
}

/// Структура, описывающая пользователя по его ID (а не логину).
///
/// Содержит:
/// - id: уникальный числовой ID (8 байт)
/// - login: строка (до 255 байт, храним длину + байты)
/// - pubkey: подпись пользователя (32 байта)
/// - device_count: количество устройств (1 байт)
/// - devices: массив устройств (все устройства фиксированной длины)
pub struct UserById {
    pub id: u64,
    pub login: String,
    pub pubkey: Pubkey,
    pub device_count: u8,
    pub devices: Vec<DeviceInfo>,
}







/// 🔧 Сериализация
/// Сериализует структуру UserById в массив байт для хранения в PDA.
///
/// Формат:
/// [0..4]      = format_type (u32)
/// [4..12]     = id (u64)
/// [12]        = длина логина (u8)
/// [13..]      = логин (байты)
/// [...]       = pubkey (32 байта)
/// [...]       = количество устройств (1 байт)
/// [..]*N      = по 65 байт на каждое устройство
pub fn serialize_user_by_id(user: &UserById) -> Vec<u8> {
    let mut result = Vec::new();

    // 1. format_type (4 байта)
    result.extend_from_slice(&USER_BY_ID_FORMAT_V1.to_le_bytes());

    // 2. id (8 байт)
    result.extend_from_slice(&user.id.to_le_bytes());

    // 3. login (длина + строка)
    let login_bytes = user.login.as_bytes();
    let login_len = login_bytes.len().min(255) as u8;
    result.push(login_len);
    result.extend_from_slice(&login_bytes[..login_len as usize]);

    // 4. pubkey (32 байта)
    result.extend_from_slice(user.pubkey.as_ref());

    // 5. количество устройств (1 байт)
    result.push(user.device_count);

    // 6. сериализуем каждое устройство (65 байт на устройство)
    for device in &user.devices {
        result.push(device.device_type);
        result.extend_from_slice(device.device_pubkey.as_ref());
        result.extend_from_slice(device.x25519_pubkey.as_ref());
    }

    result
}






/// 🔄 Общая десериализация
///
/// Десериализует UserById по переданному массиву байт.
///
/// Сначала считывает первые 4 байта как `format_type`,
/// затем вызывает нужную реализацию по формату.
pub fn deserialize_user_by_id(data: &[u8]) -> Result<UserById> {
    if data.len() < 4 {
        return Err(error!(ErrCode::DeserializationError));
    }

    let format_type = u32::from_le_bytes(data[0..4].try_into().unwrap());

    match format_type {
        USER_BY_ID_FORMAT_V1 => deserialize_user_by_id_format1(data),
        _ => Err(error!(ErrCode::UnsupportedFormat)),
    }
}









/// 🧩 Десериализация первого формата
///
/// Десериализация UserById в формате V1 (основной формат).
///
/// См. структуру сериализации выше.
fn deserialize_user_by_id_format1(data: &[u8]) -> Result<UserById> {
    let mut offset = 4; // пропускаем формат

    // 1. id
    if data.len() < offset + 8 {
        return Err(error!(ErrCode::DeserializationError));
    }
    let id = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
    offset += 8;

    // 2. login
    let login_len = data.get(offset).ok_or(ErrCode::DeserializationError)? as &u8;
    offset += 1;

    let login_end = offset + (*login_len as usize);
    if data.len() < login_end {
        return Err(error!(ErrCode::DeserializationError));
    }
    let login = std::str::from_utf8(&data[offset..login_end])
        .map_err(|_| error!(ErrCode::DeserializationError))?
        .to_string();
    offset = login_end;

    // 3. pubkey
    if data.len() < offset + 32 {
        return Err(error!(ErrCode::DeserializationError));
    }
    let pubkey = Pubkey::new_from_array(data[offset..offset + 32].try_into().unwrap());
    offset += 32;

    // 4. device_count
    if data.len() < offset + 1 {
        return Err(error!(ErrCode::DeserializationError));
    }
    let device_count = data[offset];
    offset += 1;

    // 5. devices
    let mut devices = Vec::new();
    for _ in 0..device_count {
        if data.len() < offset + 65 {
            return Err(error!(ErrCode::DeserializationError));
        }

        let device_type = data[offset];
        let device_pubkey = Pubkey::new_from_array(data[offset + 1..offset + 33].try_into().unwrap());
        let x25519_pubkey = Pubkey::new_from_array(data[offset + 33..offset + 65].try_into().unwrap());

        devices.push(DeviceInfo {
            device_type,
            device_pubkey,
            x25519_pubkey,
        });

        offset += 65;
    }

    Ok(UserById {
        id,
        login,
        pubkey,
        device_count,
        devices,
    })
}











/// ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
///             Добавление нового пользователя с одним устройством
/// ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

/// ─────────────────────────────────────────────────────────────
/// Константы для сидов PDA
/// ─────────────────────────────────────────────────────────────

/// Префикс для PDA по логину
pub const LOGIN_SEED_PREFIX: &str = "login=";

/// Префикс для PDA по ID
pub const USER_ID_SEED_PREFIX: &str = "userId=";


/// Структура аккаунтов для регистрации пользователя с одним устройством
#[derive(Accounts)]
pub struct RegisterUserWithOneDev<'info> {
    /// CHECK: Подписант (владелец логина и устройства). Проверяется вручную через `.key == &user_pubkey`
    #[account(mut, signer)]
    pub signer: AccountInfo<'info>,

    /// CHECK: PDA-счётчик количества пользователей. Проверяется вручную по сиду внутри функции
    #[account(mut)]
    pub user_counter: AccountInfo<'info>,

    /// CHECK: PDA для UserByLogin: должен быть по сиду ["login=", login]. Проверяется вручную
    #[account(mut)]
    pub user_by_login_pda: AccountInfo<'info>,

    /// CHECK: Кандидаты на PDA для UserById (всего 5 штук). Один из них должен совпасть по рассчитанному адресу
    #[account(mut)]
    pub id_pda_1: AccountInfo<'info>,
    /// CHECK: Кандидат на PDA по ID
    #[account(mut)]
    pub id_pda_2: AccountInfo<'info>,
    /// CHECK: Кандидат на PDA по ID
    #[account(mut)]
    pub id_pda_3: AccountInfo<'info>,
    /// CHECK: Кандидат на PDA по ID
    #[account(mut)]
    pub id_pda_4: AccountInfo<'info>,
    /// CHECK: Кандидат на PDA по ID
    #[account(mut)]
    pub id_pda_5: AccountInfo<'info>,

    /// Стандартная системная программа
    pub system_program: Program<'info, System>,

    /// CHECK: Получатель комиссии. Проверяется вручную по жёстко заданному адресу
    #[account(mut)]
    pub fee_receiver: AccountInfo<'info>,
}


/// ─────────────────────────────────────────────────────────────
/// Инструкция регистрации нового пользователя с одним устройством
/// ─────────────────────────────────────────────────────────────
pub fn register_user_with_one_dev(
    ctx: Context<RegisterUserWithOneDev>,
    login: String,                 // логин пользователя
    user_pubkey: Pubkey,          // публичная подпись пользователя (совпадает с signer)
    device_sign_pubkey: Pubkey,   // подпись устройства (Pubkey)
    device_x25519_pubkey: Pubkey, // ключ шифрования устройства (X25519)
) -> Result<()> {
    // ───────────── ШАГ 1 ─────────────
    // Проверка: signer должен совпадать с переданным user_pubkey

    msg!("🔐 Регистрируем пользователя с логином: {}", login);

    require!(ctx.accounts.signer.key == &user_pubkey, ErrCode::InvalidSigner);

    // ───────────── ШАГ 2 ─────────────
    // Проверка валидности логина (длина и допустимые символы)
    validate_login(&login)?;

    // ───────────── ШАГ 3 ─────────────
    // Запрещённые логины
    let reserved = ["admin", "support", "solana"];
    require!(!reserved.contains(&login.as_str()), ErrCode::InvalidLogin);

    // ───────────── ШАГ 4 ─────────────
    // Генерация PDA по логину ("login=", login)
    let login_seed_1 = LOGIN_SEED_PREFIX.as_bytes();
    let login_seed_2 = login.as_bytes();
    let (expected_login_pda, bump_login) = Pubkey::find_program_address(
        &[login_seed_1, login_seed_2], ctx.program_id);
    require!(ctx.accounts.user_by_login_pda.key == &expected_login_pda, ErrCode::InvalidPdaAddress);

    // ───────────── ШАГ 5 ─────────────
    // Проверка: PDA по логину должен быть пустым
    if ctx.accounts.user_by_login_pda.owner != &Pubkey::default() {
        return Err(error!(ErrCode::UserAlreadyExists));
    }

    // ───────────── ШАГ 6 ─────────────
    // Перевод комиссии 0.01 SOL (10_000_000 лампортов)
    let expected_receiver = Pubkey::from_str(REGISTRATION_FEE_RECEIVER)
        .map_err(|_| error!(ErrCode::InvalidLogin))?;
    require!(ctx.accounts.fee_receiver.key == &expected_receiver, ErrCode::InvalidPdaAddress);

    let ix = system_instruction::transfer(
        ctx.accounts.signer.key,
        ctx.accounts.fee_receiver.key,
        10_000_000,
    );
    invoke(&ix, &[
        ctx.accounts.signer.clone(),
        ctx.accounts.fee_receiver.clone(),
        ctx.accounts.system_program.to_account_info(),
    ])?;

    // ───────────── ШАГ 7 ─────────────
    // Получаем текущий id пользователя (из PDA-счётчика)
    let current_id = read_user_counter_pda(&ctx.accounts.user_counter, ctx.program_id)?;
    let new_id = current_id + 1;

    // ───────────── ШАГ 8 ─────────────
    // Формируем структуру UserByLogin со статусом 1
    let user_login = UserByLogin {
        login: login.clone(),
        id: new_id,
        pubkey: user_pubkey,
        status: 1,
    };
    let serialized_login = serialize_user_by_login(&user_login);

    // ───────────── ШАГ 9 ─────────────
    // Формируем структуру UserById с одним устройством
    let user_id = UserById {
        id: new_id,
        login: login.clone(),
        pubkey: user_pubkey,
        device_count: 1,
        devices: vec![DeviceInfo {
            device_type: 1,
            device_pubkey: device_sign_pubkey,
            x25519_pubkey: device_x25519_pubkey,
        }],
    };
    let serialized_id = serialize_user_by_id(&user_id);

    // ───────────── ШАГ 10 ─────────────
    // Вычисляем PDA по ID: сиды ["userId=", id as string]
    let id_seed_1 = USER_ID_SEED_PREFIX.as_bytes();
    let id_seed_2_string = new_id.to_string();            // Вначале сохраняем строку в памяти а потом преобразуем дальше
    let id_seed_2 = id_seed_2_string.as_bytes();
    let (expected_id_pda, bump_id) = Pubkey::find_program_address(
        &[id_seed_1, id_seed_2], ctx.program_id);

    let id_pdas = [
        &ctx.accounts.id_pda_1,
        &ctx.accounts.id_pda_2,
        &ctx.accounts.id_pda_3,
        &ctx.accounts.id_pda_4,
        &ctx.accounts.id_pda_5,
    ];
    let target_id_pda = id_pdas
        .iter()
        .find(|acc| acc.key == &expected_id_pda)
        .ok_or_else(|| error!(ErrCode::NoSuitableIdPda))?; // ⚠️ в будущем можно расширить систему

    // ───────────── ШАГ 11 ─────────────
    // Создаём PDA по логину и записываем туда данные
    create_pda(
        &ctx.accounts.user_by_login_pda,
        &ctx.accounts.signer,
        &ctx.accounts.system_program.to_account_info(),
        ctx.program_id,
        &[login_seed_1, login_seed_2, &[bump_login]],
        serialized_login.len() as u64,
    )?;
    write_to_pda(&ctx.accounts.user_by_login_pda, &serialized_login)?;

    // ───────────── ШАГ 12 ─────────────
    // Создаём PDA по ID и записываем туда UserById
    create_pda(
        target_id_pda,
        &ctx.accounts.signer,
        &ctx.accounts.system_program.to_account_info(),
        ctx.program_id,
        &[id_seed_1, id_seed_2, &[bump_id]],
        200,
    )?;
    write_to_pda(target_id_pda, &serialized_id)?;

    // ───────────── ШАГ 13 ─────────────
    // Обновляем счётчик пользователей
    write_user_counter_pda(&ctx.accounts.user_counter, ctx.program_id, new_id)?;

    msg!("✅ Зарегистрирован login={} id={} с 1 устройством", login, new_id);
    Ok(())
}

