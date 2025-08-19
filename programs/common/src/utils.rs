use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    program::invoke_signed,
    system_instruction,
    system_program
};






/// сдесь коды всех ошибок 

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


    #[msg("Подписавший не совпадает с ожидаемым пользователем (это потому что пока временно можно регистрировать пользователя с другово аккаунта")]
    InvalidSigner = 1005,

    /// Не получилось создат ьпользователя, система уже перегружена, попробуйте поззже!"
    #[msg("Не получилось создать пользователя, система уже перегружена, попробуйте поззже!")]
    NoSuitableIdPda = 1010,


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
        return Err(error!(ErrCode::PdaAlreadyExists));
    }

    // ───────────────────────────────────────────────
    // 2. Логируем, что будем создавать PDA
    msg!("Создаём PDA-аккаунт на {} байт", space);

    // Добавляем запас под метаданные Solana (примерно 128 байт)
    let full_space = space;

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
/// require!(!raw_bytes.is_empty(), ErrCode::EmptyPdaData);
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

    




/// ------------------------------------------------------------------------
/// delete_pda_with_assign — закрыть PDA, вернуть ренту и освободить адрес
/// ------------------------------------------------------------------------
///
/// Параметры:
/// - `pda_account`   : PDA-аккаунт (mut), который закрываем (owned вашей программой)
/// - `recipient`     : счёт, на который возвращаем лампорты (обычно пользователь)
/// - `system_program`: системная программа (111...111)
/// - `program_id`    : Pubkey вашей программы (проверка владельца)
/// - `seeds`         : сиды PDA (в том же порядке, как при создании), чтобы PDA «подписал» assign
///
/// Делает:
/// 1) Проверяет, что PDA принадлежит вашей программе.
/// 2) Обнуляет данные и сжимает их до 0 байт (realloc(0)).
/// 3) Переводит все лампорты PDA на `recipient`.
/// 4) Делает `assign` владельца на System Program (через `invoke_signed`).
///
/// Результат:
/// — В конце транзакции аккаунт с lamports=0 и data_len=0 будет удалён рантаймом,
///   владелец = System Program (чисто/ожидаемо).
/// — В следующей транзакции можно снова создать PDA с тем же сидом.
/// ------------------------------------------------------------------------

pub fn delete_pda_return_rent<'info>(
    pda_account: &AccountInfo<'info>,
    recipient: &AccountInfo<'info>,
    program_id: &Pubkey,
) -> Result<()> {
    // 0) проверки
    require!(pda_account.owner != &Pubkey::default(), ErrCode::EmptyPdaData);
    require!(pda_account.owner == program_id, ErrCode::InvalidPdaAddress);

    // 1) Переложить все лампорты с PDA на получателя (мы владелец, это разрешено)
    let amount = **pda_account.lamports.borrow();
    if amount > 0 {
        **recipient.lamports.borrow_mut() = recipient
            .lamports()
            .checked_add(amount)
            .ok_or(ProgramError::InsufficientFunds)?;
        **pda_account.lamports.borrow_mut() = 0;
    }

    // 2) Нулим данные (если были)
    if !pda_account.data_is_empty() {
        let mut data = pda_account.try_borrow_mut_data()?;
        for b in data.iter_mut() { *b = 0; }
    }

    // 3) Сжать до 0 байт
    pda_account.realloc(0, false)?;

    // Никаких assign/transfer больше не делаем — это надёжнее.
    msg!("PDA закрыт: рента отправлена на {}", recipient.key);
    Ok(())
}
 

