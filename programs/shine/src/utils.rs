use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    program::invoke_signed,
    system_instruction,
};

/// Контекст вызова test_utils
#[derive(Accounts)]
pub struct TestContext<'info> {
    /// Подписант транзакции — проверяется Anchor
    #[account(signer)]
    pub signer: AccountInfo<'info>,

    /// Аккаунт, который можно изменить
    #[account(mut)]
    pub writable_pda: AccountInfo<'info>,

    /// Аккаунт, который можно только читать
    pub readonly_pda: AccountInfo<'info>,

    /// Системная программа (нужна для создания)
    pub system_program: Program<'info, System>,
}

/// Тестовая функция — просто выводим параметры
pub fn test(
    ctx: Context<TestContext>,     // контекст с аккаунтами
    extra_pubkey: Pubkey,          // просто ключ (непроверяемый)
    number: u64,                   // число
    note: String,                  // строка
    str_array_len: u8,             // длина массива
    str_array: [String; 3],        // массив из 3 строк
) -> Result<()> {
    // Печатаем всё в лог
    msg!("Signer: {:?}", ctx.accounts.signer.key);
    msg!("Writable PDA: {:?}", ctx.accounts.writable_pda.key);
    msg!("Readonly PDA: {:?}", ctx.accounts.readonly_pda.key);
    msg!("Extra pubkey: {:?}", extra_pubkey);
    msg!("Number: {}", number);
    msg!("Note: {}", note);
    msg!("Array length: {}", str_array_len);
    for (i, s) in str_array.iter().enumerate() {
        msg!("str_array[{}] = {}", i, s);
    }

    

// ---  Пример считывания аккаунту
    let raw_bytes = safe_read_pda(&ctx.accounts.readonly_pda);
    require!(!raw_bytes.is_empty(), ErrorCode::EmptyPdaData);
    msg!("Размер считанных данных: {}", raw_bytes.len());
    
    
    // ещвщ 
// ---  Пример создания аакаунта и записи в него данных из массива
    let seeds: &[&[u8]] = &[b"test"];
    let (expected_pda, bump) = Pubkey::find_program_address(seeds, ctx.program_id);
    // Проверка PDA
    require!(ctx.accounts.writable_pda.key == &expected_pda, ErrorCode::InvalidPdaAddress);

    // Полные сиды для подписи
    let full_seeds: &[&[u8]] = &[b"test", &[bump]];

    // Запись
    create_or_write_pda(
        &ctx.accounts.writable_pda,
        &ctx.accounts.signer,
        &ctx.accounts.system_program,
        ctx.program_id,
        full_seeds,
        raw_bytes.clone(),
        raw_bytes.len() as u64,
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
  
}
#[error_code]
pub enum UserDataError {
    #[msg("Формат данных не поддерживается")]
    UnsupportedFormat = 7001,

    #[msg("Ошибка при десериализации")]
    DeserializationError = 7002,
}

///---------------------------------------------------------------------------------
///        СТРУКТУРА ПОЛЬЗОВАТЕЛЯ И ЕЁ СЕРИАЛИЗАЦИЯ И ДЕСЕРИАЛИЗАЦИЯ
/// --------------------------------------------------------------------------------
 
/// Простая структура пользователя
pub struct MyUserStruct {
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
pub fn serialize_my_user(user: &MyUserStruct) -> Vec<u8> {
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
pub fn deserialize_my_user(data: &[u8]) -> Result<MyUserStruct> {
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
pub fn deserialize_format_1(data: &[u8]) -> Result<MyUserStruct> {
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


        Ok(MyUserStruct {
            user_id,
            login,
            pubkey,
        })
    })();

    // Обернём ошибку, если любая из операций упала
    result.map_err(|_| error!(UserDataError::DeserializationError))
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
pub fn create_or_write_pda<'info>(
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
/// Эту функцию можно свободно использовать внутри инструкций:
/// ```rust,ignore
/// let bytes = safe_read_pda(&ctx.accounts.some_pda);
/// msg!("прочитано {} байт", bytes.len());
/// ```
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
