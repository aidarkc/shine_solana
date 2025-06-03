#!/bin/bash

set -e  # Завершаем при ошибке
set -o pipefail


# 🔍 Ищем запущенный solana-test-validator
EXISTING_PID=$(pgrep -f "solana-test-validator")

if [ -n "$EXISTING_PID" ]; then
  echo "🛑 Найден работающий solana-test-validator (PID $EXISTING_PID), останавливаем..."
  kill -9 $(pgrep -f "solana-test-validator")
  echo "✅ Пытаюсь остановить старый валидатор..."

  # ждём завершения
  while kill -0 "$EXISTING_PID" 2>/dev/null; do
    sleep 0.5
  done
  echo "✅ Старый валидатор остановлен."
fi

