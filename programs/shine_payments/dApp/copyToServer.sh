#!/bin/bash

#  Скрипт для копирования dApp на тестовый сервер

# Настройки
LOCAL_FILE="init.html"
REMOTE_USER="aidar"
REMOTE_HOST="shineup.me"
REMOTE_PATH="/home/aidar/Docker_server/site/dApp"

# Проверка, что файл существует
if [ ! -f "$LOCAL_FILE" ]; then
    echo "Ошибка: файл $LOCAL_FILE не найден."
    exit 1
fi

# Копирование файла
scp "$LOCAL_FILE" "${REMOTE_USER}@${REMOTE_HOST}:${REMOTE_PATH}"

# Проверка результата
if [ $? -eq 0 ]; then
    echo "Файл успешно загружен на сервер."
else
    echo "Ошибка при загрузке файла на сервер."
    exit 1
fi

#echo
#echo "Нажмите Enter, чтобы закрыть..."
#read
