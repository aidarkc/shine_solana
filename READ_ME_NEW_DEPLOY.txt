Шаг 1. Создай новый ключ для новой программы

solana-keygen new --outfile target/deploy/user_registry-testnet-keypair.json

Шаг 2. Укажи новый ID в declare_id!:

declare_id!("НОВЫЙ_PUBKEY_ОТСЮДА"); // получен из предыдущей команды

Чтобы узнать pubkey:

solana address -k target/deploy/user_registry-testnet-keypair.json

Шаг 3. Обнови Anchor.toml:

[programs.testnet]
user_registry = "НОВЫЙ_PUBKEY"

[provider]
cluster = "https://api.testnet.solana.com"
wallet = "~/.config/solana/id.json"

Шаг 4. Компиляция и деплой:

anchor build
anchor deploy --provider.cluster testnet

Шаг 5. Проверка:

solana program show НОВЫЙ_PUBKEY --url https://api.testnet.solana.com