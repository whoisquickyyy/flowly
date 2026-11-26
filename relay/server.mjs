// Лёгкий локальный/VPS WebSocket Relay для комнат Flowly
// Запуск: npm install ws && node server.mjs
import { WebSocketServer } from 'ws';
import { randomUUID } from 'crypto';

const PORT = Number(process.env.PORT || 8787);
const wss = new WebSocketServer({ port: PORT });

/** @type {Map<string, Map<string, import('ws').WebSocket>>} */
const rooms = new Map();

wss.on('connection', (ws, req) => {
  const url = new URL(req.url || '/', `http://localhost:${PORT}`);
  const roomCode = (url.searchParams.get('room') || 'DEFAULT')
    .toUpperCase()
    .replace(/[^A-Z0-9-]/g, '');
  const peerId = randomUUID();

  if (!rooms.has(roomCode)) rooms.set(roomCode, new Map());
  const peers = rooms.get(roomCode);
  peers.set(peerId, ws);

  ws.on('message', (raw) => {
    try {
      const msg = JSON.parse(raw.toString());
      msg._from = peerId;
      const payload = JSON.stringify(msg);
      for (const [id, client] of peers.entries()) {
        if (id !== peerId && client.readyState === 1) {
          client.send(payload);
        }
      }
    } catch {
      // ignore invalid json
    }
  });

  ws.on('close', () => {
    peers.delete(peerId);
    const leftMsg = JSON.stringify({ type: 'peer_left', peerId });
    for (const client of peers.values()) {
      if (client.readyState === 1) {
        client.send(leftMsg);
      }
    }
    if (peers.size === 0) rooms.delete(roomCode);
  });
});

console.log(`Flowly Jam Relay listening on ws://0.0.0.0:${PORT}`);
