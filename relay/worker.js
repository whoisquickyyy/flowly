export class JamRoom {
  constructor(state) {
    this.state = state;
    this.sessions = new Map();
  }

  async fetch(request) {
    if (request.headers.get('Upgrade') !== 'websocket') {
      return new Response('Flowly Jam WebSocket Relay', { status: 200 });
    }

    const pair = new WebSocketPair();
    const [client, server] = Object.values(pair);
    server.accept();

    const peerId = crypto.randomUUID();
    this.sessions.set(peerId, server);

    server.addEventListener('message', (event) => {
      try {
        const msg = JSON.parse(event.data);
        msg._from = peerId;
        const payload = JSON.stringify(msg);
        for (const [id, ws] of this.sessions.entries()) {
          if (id !== peerId) {
            try {
              ws.send(payload);
            } catch {
              this.sessions.delete(id);
            }
          }
        }
      } catch {
        // ignore invalid json
      }
    });

    const cleanup = () => {
      this.sessions.delete(peerId);
      const leftMsg = JSON.stringify({ type: 'peer_left', peerId });
      for (const ws of this.sessions.values()) {
        try {
          ws.send(leftMsg);
        } catch {
          // ignore
        }
      }
    };

    server.addEventListener('close', cleanup);
    server.addEventListener('error', cleanup);

    return new Response(null, { status: 101, webSocket: client });
  }
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const roomCode = (url.searchParams.get('room') || 'DEFAULT').toUpperCase().replace(/[^A-Z0-9-]/g, '');
    if (!roomCode) {
      return new Response('Missing room code', { status: 400 });
    }
    const id = env.JAM_ROOM.idFromName(roomCode);
    const stub = env.JAM_ROOM.get(id);
    return stub.fetch(request);
  },
};
