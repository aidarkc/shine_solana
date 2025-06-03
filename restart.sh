#!/bin/bash

set -e  # Завершаем при ошибке
set -o pipefail





PROGRAM_KEYPAIR="target/deploy/shine-keypair.json"  # замени на свой путь
WALLET=$(solana address)

echo "🧹 Удаление старого ledger..."
rm -rf test-ledger

echo "🚀 Запуск solana-test-validator в фоне..."
solana-test-validator --ledger test-ledger --reset > validator.log 2>&1 &
VALIDATOR_PID=$!

# Убедимся, что validator запущен
echo "⏳ Ожидание запуска валидатора..."
until solana cluster-version &>/dev/null; do
  sleep 1
done
sleep 2  # На всякий случай немного подождём

echo "💸 Airdrop 10 SOL на $WALLET..."
solana airdrop 10 $WALLET

echo "🔨 Сборка контракта..."
anchor build

echo "📦 Деплой контракта..."
anchor deploy

echo "✅ Готово!"

# Не убиваем валидатор, чтобы он оставался запущенным
echo "ℹ️ Валидатор всё ещё работает (PID $VALIDATOR_PID)"
echo "Нажмите Ctrl+C, чтобы остановить его."
wait $VALIDATOR_PID
