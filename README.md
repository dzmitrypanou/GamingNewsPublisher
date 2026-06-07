# Gaming News Publisher

Десктоп-приложение для автоматического сбора игровых новостей из RSS, переписывания через DeepSeek AI и публикации в VK и Telegram.

## Возможности

- **RSS-парсинг** — сбор новостей из настраиваемых источников (встроенные пресеты: IGN, PC Gamer, Eurogamer и др.)
- **DeepSeek AI** — автоматическая генерация коротких заголовков, текста и хештегов
- **Публикация** — одновременная отправка в VK (группа) и Telegram (канал) с картинкой
- **Редактор** — предпросмотр поста для VK и Telegram side-by-side, ручная правка
- **Категории** — PC, Консоли, Мобильные, Киберспорт, Инди, Анонсы, Обзоры
- **Автоматизация** — фоновый парсинг по интервалу, опциональная автопубликация

## Требования

- **Node.js** 18+
- **Rust** 1.77+ ([установка](https://www.rust-lang.org/tools/install))
- **Windows 10/11** (WebView2 установлен по умолчанию)

### Установка Rust (Windows)

```powershell
winget install Rustlang.Rustup
```

После установки перезапустите терминал и проверьте:

```powershell
rustc --version
cargo --version
```

## Установка и запуск

```powershell
cd d:\Social
npm install
npm run tauri dev
```

Сборка установщика:

```powershell
npm run tauri build
```

## Portable .exe (без консольных окон)

Собрать папку с тихим launcher и всем необходимым:

```powershell
npm run build:portable
```

Результат: `GamingNewsPublisher/`

| Файл | Назначение |
|------|------------|
| `Gaming News Publisher.exe` | Тихий launcher — запускает приложение без cmd-окна |
| `app\gaming-news-publisher.exe` | Основная программа |
| `Gaming News Publisher_*_setup.exe` | Установщик с WebView2 (для других ПК) |

Скопируйте всю папку `GamingNewsPublisher` куда угодно и запускайте через `Gaming News Publisher.exe`.

## Первый запуск

1. Откройте **Настройки** и введите API-ключи (см. ниже)
2. Нажмите **Проверить** для каждой платформы
3. Перейдите в **Источники** — добавьте RSS-фиды или выберите предустановленные
4. На **Дашборде** нажмите **Собрать новости**
5. В **Постах** отредактируйте и опубликуйте

## Получение API-ключей

### VKontakte (два токена)

1. **Ключ сообщества** — Управление группы → Работа с API → Ключи доступа (wall, photos, управление). Нужен для публикации постов от имени группы.
2. **User token** (опционально) — токен администратора группы для загрузки фото. Ключ сообщества фото через API не загружает.
3. **ID группы** — число без минуса, например `188809704`.

User token: старое Standalone-приложение в [dev.vk.com](https://dev.vk.com) → Мои приложения, scope: `wall`, `photos`, `groups`, `offline`. Получить через OAuth (Implicit Flow) под аккаунтом администратора группы.

### Telegram

1. Создайте бота через [@BotFather](https://t.me/BotFather) — получите Bot Token
2. Создайте канал и добавьте бота как **администратора**
3. Укажите Channel ID: `@yourchannel` или числовой `-1001234567890`

### DeepSeek

1. Зарегистрируйтесь на [platform.deepseek.com](https://platform.deepseek.com)
2. Создайте API Key в разделе API Keys
3. Укажите ключ в настройках (модель по умолчанию: `deepseek-chat`)

## Структура проекта

```
src/              — React UI (TypeScript + Tailwind)
src-tauri/        — Rust backend (Tauri 2)
  src/commands/   — Tauri IPC-команды
  src/services/   — VK, Telegram, DeepSeek, RSS
  src/db/         — SQLite
```

## Хранение данных

- **API-ключи** — локальное зашифрованное хранилище (`tauri-plugin-store`)
- **Посты, источники, категории** — SQLite (`%APPDATA%/com.gamingnews.publisher/`)

## Лицензия

MIT
