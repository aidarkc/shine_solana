# подключаться надо к
JSON RPC URL: http://127.0.0.1:8899

# Запустить саму ноду
solana-test-validator
# Удалить все данные с ноды
solana-test-validator --reset


# Что бы запустить просмотр логов ноды
solana logs

# Запустить контракт
anchor deploy

# Cкомпилировать и задеплоить новую версию
anchor build                     # Скомпилировать контракт и сгенерировать IDL
anchor deploy                    # Задеплоить контракт в сеть (указанную в Anchor.toml)
Если ты хочешь сразу убедиться, куда он деплоится — проверь Anchor.toml.
[provider]
cluster = "https://api.testnet.solana.com"  # или "localnet"
wallet = "~/.config/solana/id.json"




# Создать новый проект
anchor init имя_проекта
