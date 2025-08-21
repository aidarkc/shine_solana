#!/bin/bash

set -e  # Завершаем при ошибке
set -o pipefail


kill -9 $(pgrep -f "solana-test-validator")

# 🔍 Ищем запущенный solana-test-validator
EXISTING_PID=$(pgrep -f "solana-test-validator")

if [ -n "$EXISTING_PID" ]; then
  echo "🛑 Найден работающий solana-test-validator (PID $EXISTING_PID), останавливаем..."
  bash kill -9 $(pgrep -f "solana-test-validator")
  echo "✅ Пытаюсь остановить старый валидатор..."

  # ждём завершения
  while kill -0 "$EXISTING_PID" 2>/dev/null; do
    sleep 0.5
  done
  echo "✅ Старый валидатор остановлен."
fi

