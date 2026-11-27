# Flowly (Aura Music) — Архитектура и структура проекта

Мультистриминговый десктопный музыкальный плеер на **Tauri v2 (Rust)** + **Svelte 5 (Runes)** + **Vite**.  
Объединяет **Яндекс Музыку**, **SoundCloud**, локальную библиотеку, **«Мою волну»**, **Jam-комнаты**, 10-полосный эквалайзер и реактивные фоновые сцены.

---

## Стек и команды сборки

- **Фронтенд**: Svelte 5 (`$state`, `$derived`, `$effect`), TypeScript, `bits-ui`, `lucide-svelte`, Web Audio API.
- **Бэкенд**: Rust (Tauri 2), `tokio`, `reqwest` (Direct / SOCKS5 / HTTP прокси + DoH), `lofty` (метаданные локальных треков), `souvlaki` (SMTC), `discord-rich-presence`.
- **Проверка и сборка**:
  ```bash
  npm run check      # svelte-check --tsconfig ./tsconfig.json
  npm run build      # vite build
  cd src-tauri && cargo check
  ```

---

## Структура директорий

### Фронтенд (`src/`)

- `src/App.svelte` — главный оркестратор приложения:
  - Состояние плеера (`player`), две HTMLAudioElement-деки (`audioA` / `audioB`) для бесшовного кроссфейда и предзагрузки.
  - Обработка Jam-комнаты (WebSocket / Supabase Realtime, `realtime:jam-<code>` и лобби `realtime:jam-lobby`).
  - Навигация по вкладкам (`details`, `library`, `playlists`, `search`, `recap`, `room`, `equalizer`, `settings`) и карточке артиста (`activeArtist`).
- `src/lib/audioEngine.ts` — Web Audio API DSP-движок:
  - Две входные деки (`deckGainA/B`, `deckFilterA/B`) → `preampGain` → 10-полосный параметрический EQ → `bassFilter` → `compressor` → `makeupGain` → **Spatial 3D-матрица (Mid-Side расширение + бинауральная комната)** → `limiter` → `analyser` (FFT).
  - Кроссфейды `crossfadeTo(target, durationSec, preset, curve)` (`equal-power`, `s-curve`, `linear`, `rise`) и `beatmatchTransition`.
- `src/lib/api.ts` — типизированная обёртка над `invoke(...)` для всех Tauri-команд.
- `src/lib/types.ts` — общие TypeScript-интерфейсы (`Track`, `PlayerState`, `AudioEffectsConfig`, `MixConfig`, `ArtistYandexInfo`, `Playlist`, `SourceStatus` и др.).
- `src/lib/jamRoom.ts` — чистая логика Jam-комнат: генерация кодов, сборка общего микса (`buildBlendedQueue`), типы состояния комнаты и открытого лобби.
- `src/lib/camelot.ts` — определение BPM/тональности (колесо Камелота) и расчёт гармонического кроссфейда.
- `src/lib/colorExtract.ts` — извлечение `CoverPalette` из обложки трека.
- `src/lib/supabase.ts` — профиль Flowly, Supabase Auth и URL для Supabase Realtime.
- `src/lib/components/`:
  - `DetailsView.svelte` — главный экран «Сейчас играет», «Моя волна», быстрый блок «Медиатека», тексты песен, сэмплы, TV-режим.
  - `ArtistView.svelte` — карточка артиста в стиле Яндекс Музыки (Hero-баннер, табы «Обзор / Треки / Альбомы / Синглы и EP / Похожие», встроенный просмотрщик альбома).
  - `EqualizerPanel.svelte` — 10-полосный EQ, пресеты, настройки Mix & Auto-Mix переходов, Spatial 3D и Bass Boost.
  - `RoomView.svelte` — интерфейс Jam-комнаты (общие/приватные комнаты, открытое лобби, вклад в микс, чат).
  - `SettingsView.svelte` — аккаунты сервисов (Яндекс Музыка, SoundCloud OAuth 2.1 через браузер), прокси для SoundCloud и DoH (вкладка «Сеть и DoH»), темы, звук и обновления.
  - `LibraryView.svelte`, `PlaylistsView.svelte`, `SearchView.svelte`, `RecapView.svelte`, `ReactiveBackground.svelte`, `ui/` (`RadixSelect`, `RadixSlider`, `RadixSwitch`).

### Бэкенд (`src-tauri/src/`)

- `lib.rs` — инициализация Tauri-плагинов (`updater`, `process`, `opener`), системного трея, `AppState`, локального `proxy_server`, SMTC и регистрация `invoke_handler`.
- `player.rs` — `AppState` (состояние плеера, `ProviderRegistry`, `SecretStore`, `RecapStore`, `TrackCache`, `sc_oauth`).
- `auth/mod.rs` — `SecretStore` (`secrets.json` в `app_data_dir`), `SoundCloudOAuthState` и `SoundCloudOAuthStatus` (единый источник правды для OAuth-сессии SoundCloud и `sessionId`).
- `commands.rs` — все `#[tauri::command]` эндпоинты:
  - Управление плеером, очередью, «Моей волной», лайками, плейлистами, локальными папками (включая фоновый прогрев `playback_url` последнего трека после холодного старта).
  - Авторизация Яндекс Музыки и SoundCloud (`start_soundcloud_oauth` с PKCE + Cloudflare Worker `relay/soundcloud-oauth-worker.js` и `tauri-plugin-opener`, `poll_soundcloud_oauth` с опросом каждые 700 мс, `cancel_soundcloud_oauth`, `connect_soundcloud`).
- `providers/`:
  - `mod.rs` — `ProviderRegistry` (агрегация поиска, библиотеки, волны и стриминга по источникам).
  - `yandex.rs` — клиент Яндекс Музыки (поиск, волна, альбомы, `brief-info` артиста, тексты, обложки).
  - `soundcloud.rs` — клиент SoundCloud (официальный OAuth 2.1 `api.soundcloud.com` с `Bearer`-токеном + fallback на `api-v2`, автоматический `client_id`, лайки, плейлисты, поиск, стримы).
- `relay/` — внешние ретрансляторы:
  - `soundcloud-oauth-worker.js` + `wrangler-sc.toml` — Cloudflare Worker (KV `SC_SESSIONS`) для SoundCloud OAuth 2.1 PKCE и проксирования API/стримов SoundCloud (`/sc-proxy`), благодаря чему SoundCloud работает в режиме `Direct` без настройки прокси/VPN.
  - `worker.js` + `server.mjs` — WebSocket Relay для Jam-комнат.
- `proxy_server.rs` — локальный HTTP-прокси (`127.0.0.1:<port>`) для стриминга удалённых треков через настроенный прокси/DoH и отдачи локальных файлов (`/local`).
- `network/` — сборка `reqwest::Client` с SOCKS5/HTTP прокси и DoH-резолвером.
- `audio/` — зарезервированный модуль `kithara_engine.rs` (на данный момент воспроизведение и DSP работают через Web Audio API в `src/lib/audioEngine.ts`).
- `lyrics.rs`, `recap.rs`, `samples.rs`, `radio_host.rs`, `discord_rpc.rs`, `media_controls.rs`, `local_library.rs`, `track_cache.rs` — вспомогательные подсистемы.
