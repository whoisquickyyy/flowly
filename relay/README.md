# Flowly Jam WebSocket Relay

Лёгкий ретранслятор комнат для совместного прослушивания во Flowly. Аудио через него не идёт — передаются только метаданные треков, очередь и таймкоды синхронизации.

## Вариант 1: Бесплатный деплой в интернет (Cloudflare Workers)
```bash
cd relay
npx wrangler deploy
```
После деплоя вставьте выданный адрес (заменив `https://` на `wss://`, например `wss://flowly-jam-relay.yourname.workers.dev`) в поле **Relay URL** во вкладке «Комната» в Flowly.

## Вариант 2: Локальный запуск или свой VPS (Node.js)
```bash
cd relay
npm init -y && npm install ws
node server.mjs
```
По умолчанию слушает `ws://127.0.0.1:8787` (или `wss://ваш-домен` за nginx/caddy).
