Devnet E2E тест: NFT-модуль + add_bonus

Ветка содержит скрипты для проверки (NFT + add_bonus) в devnet.

Скрипты:

quick_devnet_e2e.js — создаёт 1 NFT и вызывает add_bonus.

quick_devnet_e2e_multi.js — создаёт N NFT в коллекции.



Подготовка окружения

Установите зависимости:

npm i @coral-xyz/anchor @solana/web3.js @solana/spl-token


Создайте файл .env с переменными:

ANCHOR_PROVIDER_URL=https://api.devnet.solana.com
ANCHOR_WALLET=/Users/<user>/.config/solana/id.json
PROGRAM_ID=<адрес shine_payments в devnet>
COLLECTION_MINT=<mint коллекции>


Пополните кошелёк тестовыми SOL:

solana config set --url https://api.devnet.solana.com
solana airdrop 2

Запуск тестов:

одиночный NFT:

node quick_devnet_e2e.js


несколько NFT:

node quick_devnet_e2e_multi.js 3



Проверка результата

В выводе будут строки:

add_bonus() tx: <signature>

NFT mint: <address>



Откройте транзакцию или mint в Solana Explorer


Убедитесь:

Verified Collection совпадает с вашим COLLECTION_MINT

У каждого NFT Supply = 1

Аккаунт получателя (ATA) помечен как frozen




⚠️ Повторный запуск может вернуть ошибку PdaAlreadyExists это нормально, так как PDA уже инициализирован.
