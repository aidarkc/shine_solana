use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    program::invoke_signed,
    system_instruction,
};

const USER_SEED_PREFIX: &str = "u=";

/// Контекст вызова test_utils
#[derive(Accounts)]
pub struct TestContext<'info> {
    /// Подписант транзакции — проверяется Anchor
    /// CHECK: Только для чтения. Никаких операций записи не производится.
    #[account(signer)]
    pub signer: AccountInfo<'info>,

    /// Аккаунт, который можно изменить
    /// CHECK: Это PDA, чья валидность проверяется через seeds и signer
    #[account(mut)]
    pub writable_pda: AccountInfo<'info>,

    /// Аккаунт, который можно только читать
    /// CHECK: Только для чтения. Никаких операций записи не производится.
    pub readonly_pda: AccountInfo<'info>,

    /// Системная программа (нужна для создания)/// 
    /// CHECK: Только для чтения. Никаких операций записи не производится.
    pub system_program: Program<'info, System>,}

/// Тестовая функция — просто выводим параметры
pub fn test(
    ctx: Context<TestContext>,     // контекст с аккаунтами
    extra_pubkey: Pubkey,          // просто ключ (непроверяемый)
    number: u64,                   // число
    note: String,                  // строка
    str_array: Vec<String>,        // массив строк переменной длинны
) -> Result<()> {
    // Печатаем всё в лог
    msg!("Signer: {:?}", ctx.accounts.signer.key);
    msg!("Writable PDA: {:?}", ctx.accounts.writable_pda.key);
    msg!("Readonly PDA: {:?}", ctx.accounts.readonly_pda.key);
    msg!("Extra pubkey: {:?}", extra_pubkey);
    msg!("Number: {}", number);
    msg!("Note: {}", note);
    msg!("Array length: {}", str_array.len());
    for (i, s) in str_array.iter().enumerate() {
        msg!("str_array[{}] = {}", i, s);
    }

    

// ---  Пример считывания аккаунту
    
    let raw_bytes = safe_read_pda(&ctx.accounts.readonly_pda);
    // Проверяем, что массив не пустой
    require!(!raw_bytes.is_empty(), ErrorCode::EmptyPdaData);
    msg!("Размер считанных данных: {}", raw_bytes.len());

    // ───────────────────────────────
    // Пробуем десериализовать данные
    let user = deserialize_my_user(&*raw_bytes)?;

    // ───────────────────────────────
    // Выводим логин пользователя
    msg!("✅ Десериализация успешна, логин: {}", user.login);    
    
    // Печатаем массив по байтам: [00 2A FF ...]
    let mut output = String::new();
    for (i, byte) in raw_bytes.iter().enumerate() {
        use std::fmt::Write;

        if i % 16 == 0 {
            // Новая строка с адресом (опционально)
            let _ = write!(output, "\n{:04X}: ", i);
        }
        let _ = write!(output, "{:02X} ", byte);
    }
    msg!("📦 Данные PDA:{}", output);


    
    
    
    
    
    
// --- пример создания объекта UserStruct его и сериализации    
    // --- Создаём объект MyUserStruct
    let user_struct = UserStruct {
        user_id: number,                               // любое тестовое значение
        login: note.clone(),                        // можно передать note.clone() или строку из массива
        pubkey: ctx.accounts.signer.key().clone(),  // например, signer
    };

    // --- Сериализуем в массив байт
     let serialized_bytes = serialize_my_user(&user_struct);



    // ---  Пример создания PDA и записи в него данных из массива
    let seed_string = format!("{}{}", USER_SEED_PREFIX, note);
    let seed_bytes = seed_string.as_bytes();

        
    // Поиск PDA
    let seeds: &[&[u8]] = &[seed_bytes];
    let (expected_pda, bump) = Pubkey::find_program_address(seeds, ctx.program_id);
    // Проверка PDA
    require!(ctx.accounts.writable_pda.key == &expected_pda, ErrorCode::InvalidPdaAddress);

    // Полные сиды для подписи
    let full_seeds: &[&[u8]] = &[seed_bytes, &[bump]];

    msg!("serialized_bytes.len() as u64 {}", serialized_bytes.len() as u64);
    // Запись
    create_and_write_pda(
        &ctx.accounts.writable_pda,
        &ctx.accounts.signer,
        &ctx.accounts.system_program,
        ctx.program_id,
        full_seeds,
        serialized_bytes.clone(),
        serialized_bytes.len() as u64,
    )?;
    
    
    Ok(())
}







/// сдесь коды всех ошибок 

#[error_code]
pub enum ErrorCode {
    #[msg("PDA не содержит данных или не инициализирован")]
    EmptyPdaData = 6002,

    #[msg("Пользователь уже зарегистрирован")]
    UserAlreadyExists = 6003,

    #[msg("Некорректный логин")]
    InvalidLogin = 6004,

    #[msg("Не совпадает PDA адрес")]
    InvalidPdaAddress = 6006,

    #[msg("Формат данных не поддерживается")]
    UnsupportedFormat = 7001,

    #[msg("Ошибка при десериализации")]
    DeserializationError = 7002,

    /// PDA уже существует, создание невозможно
    #[msg("PDA-аккаунт уже существует и не может быть создан повторно.")]
    PdaAlreadyExists = 1009,

    /// Система уже инициализирована и не может быть инициализирована повторно!
    #[msg("Система уже инициализирована и не может быть инициализирована повторно!")]
    SystemAlreadyInitialized = 4000,
}
#[error_code]
pub enum UserDataError {
    #[msg("Формат данных не поддерживается")]
    UnsupportedFormat = 7001,

    #[msg("Ошибка при десериализации")]
    DeserializationError = 7002,
}

///---------------------------------------------------------------------------------
/// 
///   СТРУКТУРА ПОЛЬЗОВАТЕЛЯ И ЕЁ СЕРИАЛИЗАЦИЯ И ДЕСЕРИАЛИЗАЦИЯ
/// 
/// --------------------------------------------------------------------------------
 
/// Простая структура пользователя
pub struct UserStruct {
    //pub format_type: u32,   // 4 байта        не храним
    pub user_id: u64,       // 8 байт
    pub login: String,      // сначала длина, потом байты
    pub pubkey: Pubkey,     // 32 байта
}

///    ------------  Сериализация -----------
// Пример использования

// let user_bytes = serialize_my_user(&some_user);
// match deserialize_my_user(&user_bytes) {
// Ok(user) => msg!("Десериализован логин: {}", user.login),
// Err(e) => msg!("Ошибка: {}", e),
// }

/// Метод сериализации в Vec<u8>
/// let user = MyUserStruct {
///     format_type: 1,
///     user_id: 42,
///     login: String::from("sol_user"),
///     pubkey: Pubkey::new_unique(),
/// };
/// 
/// let bytes = serialize_my_user(&user);
/// msg!("Сериализовано {} байт: {:?}", bytes.len(), bytes);
pub fn serialize_my_user(user: &UserStruct) -> Vec<u8> {
    let mut result = Vec::new();

    // ───────────────
    // 1. format_type (4 байта)
    // ───────────────
    result.extend_from_slice(&1u32.to_le_bytes());
    //result.extend_from_slice(&user.format_type.to_le_bytes());  не храним

    // ───────────────
    // 2. user_id (8 байт)
    // ───────────────
    result.extend_from_slice(&user.user_id.to_le_bytes());

    // ───────────────
    // 3. login: сначала длина (u8), затем байты
    // ───────────────
    let login_bytes = user.login.as_bytes();
    let login_len = login_bytes.len();

    // Если логин длиннее 255 символов — ошибка (или обрежем)
    let login_len_u8 = login_len.min(255) as u8;

    result.push(login_len_u8); // длина
    result.extend_from_slice(&login_bytes[..login_len_u8 as usize]); // строка

    // ───────────────
    // 4. Pubkey (32 байта)
    // ───────────────
    result.extend_from_slice(user.pubkey.as_ref());

    result
}
///    ------------  Десериализация -----------

/// Основная функция: определяет формат и вызывает нужную десериализацию
pub fn deserialize_my_user(data: &[u8]) -> Result<UserStruct> {
    // Проверяем, что в байтах хотя бы 4 байта под формат
    if data.len() < 4 {
        return Err(error!(UserDataError::DeserializationError));
    }

    // Читаем первые 4 байта — тип формата
    let format_type = u32::from_le_bytes(data[0..4].try_into().map_err(|_| UserDataError::DeserializationError)?);

    // Ветвление по типу формата
    match format_type {
        1 => deserialize_format_1(data),
        // 2 => deserialize_format_2(data),
        // 3 => ...
        _ => Err(error!(UserDataError::UnsupportedFormat)),
    }
}

/// Десериализация данных формата 1:
/// [0..4]  → format_type: u32
/// [4..12] → user_id: u64
/// [12..13] → длина логина: u8
/// [13..(13+len)] → логин
/// [..] → 32 байта pubkey
pub fn deserialize_format_1(data: &[u8]) -> Result<UserStruct> {
    // Оборачиваем всё в одну try-блок, чтобы перехватить любые ошибки
    let result = (|| {
        if data.len() < 4 + 8 + 1 + 32 {
            return Err(UserDataError::DeserializationError);
        }

        //let format_type = u32::from_le_bytes(data[0..4].try_into().unwrap());  не храним
        let user_id = u64::from_le_bytes(data[4..12].try_into().unwrap());

        let login_len = data[12] as usize;
        let login_start = 13;
        let login_end = login_start + login_len;

        if data.len() < login_end + 32 {
            return Err(UserDataError::DeserializationError);
        }

        let login_bytes = &data[login_start..login_end];
        let login = std::str::from_utf8(login_bytes)
            .map_err(|_| UserDataError::DeserializationError)?
            .to_string();

        let pubkey_start = login_end;
        let pubkey_end = pubkey_start + 32;
        let pubkey = Pubkey::try_from(&data[pubkey_start..pubkey_end])
            .map_err(|_| UserDataError::DeserializationError)?;


        Ok(UserStruct {
            user_id,
            login,
            pubkey,
        })
    })();

    // Обернём ошибку, если любая из операций упала
    result.map_err(|_| error!(UserDataError::DeserializationError))
}










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
        return Err(error!(UserDataError::DeserializationError));
    }

    // Считываем format_type
    let format_type = u32::from_le_bytes(data[0..4].try_into().unwrap());

    match format_type {
        1 => deserialize_user_by_login_format1(data),
        _ => Err(error!(UserDataError::UnsupportedFormat)),
    }
}

/// ───────────────────────────────────────────────────────────────────────
/// Распаковываем user_by_login формат 1:
/// ───────────────────────────────────────────────────────────────────────
fn deserialize_user_by_login_format1(data: &[u8]) -> Result<UserByLogin> {
    let mut offset = 4; // пропускаем format_type

    // 1. login (длина + строка)
    let login_len = data.get(offset).ok_or(UserDataError::DeserializationError)? as &u8;
    offset += 1;

    let login_end = offset + (*login_len as usize);
    if data.len() < login_end {
        return Err(error!(UserDataError::DeserializationError));
    }

    let login = std::str::from_utf8(&data[offset..login_end])
        .map_err(|_| error!(UserDataError::DeserializationError))?
        .to_string();
    offset = login_end;

    // 2. id (u64)
    if data.len() < offset + 8 {
        return Err(error!(UserDataError::DeserializationError));
    }
    let id = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
    offset += 8;

    // 3. pubkey (32 байта)
    if data.len() < offset + 32 {
        return Err(error!(UserDataError::DeserializationError));
    }
    let pubkey = Pubkey::new_from_array(data[offset..offset + 32].try_into().unwrap());
    offset += 32;

    // 4. status (u32)
    if data.len() < offset + 4 {
        return Err(error!(UserDataError::DeserializationError));
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
    require!(counter_pda.key == &expected_pda, ErrorCode::InvalidPdaAddress);

    // Безопасное чтение данных
    let raw = safe_read_pda(counter_pda);
    if raw.len() != 8 {
        return Err(error!(ErrorCode::EmptyPdaData)); // неверный размер
    }

    // Преобразуем 8 байт в u64
    let value = u64::from_le_bytes(raw.try_into().map_err(|_| ErrorCode::DeserializationError)?);
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
    require!(counter_pda.key == &expected_pda, ErrorCode::InvalidPdaAddress);

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
    require!(counter_pda.key == &expected_pda, ErrorCode::InvalidPdaAddress);

    // Проверка — если PDA уже существует, завершаем с ошибкой
    if counter_pda.owner != &Pubkey::default() {
        msg!("PDA Со счётчиком пользователей уже существует. Система уже инициализированна!");
        return Err(error!(ErrorCode::SystemAlreadyInitialized));
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












///----------------------------------------------------------------------------------------------------------
///   Создание чтение  PDA
///----------------------------------------------------------------------------------------------------------

/// Создаёт PDA аккаунт (если его ещё нет), и записывает в него массив байт.
///
/// Аргументы:
/// - `pda_account`: аккаунт, куда записываем
/// - `signer`: кто платит за создание (обычно пользователь)
/// - `program_id`: адрес текущей программы
/// - `seeds`: слайс сидов, по которым создавался PDA
/// - `data`: байты для записи
/// - `space`: желаемый размер аккаунта
pub fn create_and_write_pda<'info>(
    pda_account: &AccountInfo<'info>,
    signer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    program_id: &Pubkey,
    seeds: &[&[u8]],
    data: Vec<u8>,
    space: u64,
) -> Result<()> {
    // ───────────────────────────────────────────────
    // 1. Проверяем, создан ли аккаунт (если нет — owner = default)
    if pda_account.owner == &Pubkey::default() {
        msg!("Создаём PDA с размером {} байт", space);

        let space = space; //+ 128; // Добавляется запас под метаданные
        // Вычисляем необходимую арендную плату
        let lamports = Rent::get()?.minimum_balance(space as usize);

        // Формируем инструкцию
        let create_instr = system_instruction::create_account(
            signer.key,
            pda_account.key,
            lamports,
            space,
            program_id,
        );

        // Выполняем инструкцию с подписью от PDA
        invoke_signed(
            &create_instr,
            &[
                signer.clone(),
                pda_account.clone(),
                system_program.clone(),
            ],
            &[&seeds],
        )?;
    }

    // ───────────────────────────────────────────────
    // 2. Пишем данные в аккаунт
    let mut account_data = pda_account.try_borrow_mut_data()?;

    let copy_len = std::cmp::min(account_data.len(), data.len());
    account_data[..copy_len].copy_from_slice(&data[..copy_len]);

    // Если хочешь дополнить оставшееся нулями — раскомментируй:
    // for i in copy_len..account_data.len() {
    //     account_data[i] = 0;
    // }

    msg!("Успешно записано {} байт в PDA", copy_len);
    Ok(())
}




/// Создаёт PDA аккаунт (если его ещё нет).
///
/// ⚠️ Если аккаунт уже существует, выбрасывается ошибка.
/// Используется внутри инструкций смарт-контракта.
///
/// Аргументы:
/// - `pda_account`: аккаунт, который хотим создать (PDA)
/// - `signer`: кто оплачивает создание аккаунта (обычно пользователь)
/// - `system_program`: системная программа (`111...111`)
/// - `program_id`: адрес текущей программы (используется для подписи PDA)
/// - `seeds`: массив сидов, по которым вычислялся PDA
/// - `space`: желаемый размер аккаунта в байтах (только данных, без метаданных)
pub fn create_pda<'info>(
    pda_account: &AccountInfo<'info>,
    signer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    program_id: &Pubkey,
    seeds: &[&[u8]],
    space: u64,
) -> Result<()> {
    // ───────────────────────────────────────────────
    // 1. Проверяем, существует ли аккаунт
    if pda_account.owner != &Pubkey::default() {
        // Если владелец не равен Pubkey::default, значит аккаунт уже создан
        // Возвращаем ошибку с пояснением
        return Err(error!(ErrorCode::PdaAlreadyExists));
    }

    // ───────────────────────────────────────────────
    // 2. Логируем, что будем создавать PDA
    msg!("Создаём PDA-аккаунт на {} байт", space);

    // Добавляем запас под метаданные Solana (примерно 128 байт)
    let full_space = space + 128;

    // Получаем минимальный баланс для аренды (чтобы аккаунт не удалили)
    let lamports = Rent::get()?.minimum_balance(full_space as usize);

    // ───────────────────────────────────────────────
    // 3. Создаём инструкцию system_program для создания аккаунта
    let create_instr = system_instruction::create_account(
        signer.key,         // от имени кого
        pda_account.key,    // для какого PDA
        lamports,           // сколько лампортов перевести
        full_space,         // сколько байт выделить
        program_id,         // кто будет владельцем PDA
    );

    // ───────────────────────────────────────────────
    // 4. Выполняем инструкцию с подписью PDA (через сиды)
    invoke_signed(
        &create_instr,
        &[
            signer.clone(),
            pda_account.clone(),
            system_program.clone(),
        ],
        &[&seeds], // PDA сиды → для подписи
    )?;

    Ok(())
}

/// Записывает массив байт в PDA аккаунт (в начало data-секции).
///
/// ⚠️ Убедись, что PDA был передан как `#[account(mut)]`
/// ⚠️ Эта функция ничего не создаёт, только пишет.
///
/// Аргументы:
/// - `pda_account`: аккаунт, в который пишем (должен быть mut)
/// - `data`: бинарный массив, который нужно записать
pub fn write_to_pda<'info>(
    pda_account: &AccountInfo<'info>,
    data: &[u8],
) -> Result<()> {
    // ───────────────────────────────────────────────
    // 1. Получаем доступ к данным PDA (на запись)
    let mut account_data = pda_account.try_borrow_mut_data()?;

    // ───────────────────────────────────────────────
    // 2. Вычисляем сколько байт реально можно записать
    // (на случай, если data длиннее, чем выделено место)
    let copy_len = std::cmp::min(account_data.len(), data.len());

    // ───────────────────────────────────────────────
    // 3. Копируем данные в аккаунт (с самого начала)
    account_data[..copy_len].copy_from_slice(&data[..copy_len]);

    // Логируем, сколько байт записано
    msg!("Успешно записано {} байт в PDA", copy_len);

    Ok(())
}










/// ------------------------------------------------------------------------
/// safe_read_pda ‒ «безопасное чтение PDA»
/// ------------------------------------------------------------------------
///
/// * Принимает:   ссылку на `AccountInfo<'info>` PDA-аккаунта.
/// * Возвращает:  `Vec<u8>` с данными аккаунта.  
///                Если аккаунта нет или его данные пусты — возвращается `Vec::new()`
///                длиной 0 байт.
///
/// Как работает ───────────────────────────────────────────────────────────
/// 1. Проверяем, что аккаунт **инициализирован**: у не-инициализированного
///    owner = Pubkey::default(). Если owner нулевой — сразу отдаём пустой вектор.
/// 2. Если длина буфера == 0 (Anchor helper `data_is_empty()`), тоже отдаём пустой.
/// 3. Пытаемся безопасно (`try_borrow_data`) получить ссылку на данные.
///    - Успех → копируем их в Vec и возвращаем.
///    - Ошибка (например, конфликт borrow) → логируем и возвращаем пустой Vec.
///
/// пример использования 
/// let raw_bytes = safe_read_pda(&ctx.accounts.readonly_pda);
/// require!(!raw_bytes.is_empty(), ErrorCode::EmptyPdaData);
/// msg!("Размер считанных данных: {}", raw_bytes.len());
/// ------------------------------------------------------------------------
pub fn safe_read_pda<'info>(pda_account: &AccountInfo<'info>) -> Vec<u8> {
    // ─────────────────────────────────────────────────────────────────────
    // 1) Аккаунт Н*Е* СУЩЕСТВУЕТ или не инициализирован:
    // owner == Pubkey::default() (в Solana нулевой owner у пустого счёта)
    // ─────────────────────────────────────────────────────────────────────
    if pda_account.owner == &Pubkey::default() {
        msg!("safe_read_pda: аккаунт не инициализирован ‒ возвращаем пустой массив");
        return Vec::new(); // []
    }

    // ─────────────────────────────────────────────────────────────────────
    // 2) У аккаунта нет данных (длина 0) — тоже считаем «пустым»
    // ─────────────────────────────────────────────────────────────────────
    if pda_account.data_is_empty() {
        msg!("safe_read_pda: у аккаунта data_len == 0 ‒ возвращаем пустой массив");
        return Vec::new();
    }

    // ─────────────────────────────────────────────────────────────────────
    // 3) Пытаемся безопасно забрать буфер данных; ошибки перехватываем
    // ─────────────────────────────────────────────────────────────────────
    match pda_account.try_borrow_data() {
        Ok(data_ref) => {
            // to_vec() копирует bytes → Vec<u8>, чтобы дальше работать без borrow-лифа
            data_ref.to_vec()
        }
        Err(e) => {
            // Ошибка при borrow (например, уже есть активное мутабельное заимствование)
            msg!("safe_read_pda: ошибка borrow_data ({:?}) ‒ возвращаем пустой массив", e);
            Vec::new()
        }
    }
}
