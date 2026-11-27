/**
 * Cloudflare Worker для SoundCloud OAuth 2.1 (PKCE) — бэкенд-ретранслятор для Flowly.
 *
 * Деплой:
 *   1. Создайте KV-пространство: npx wrangler kv namespace create SC_SESSIONS
 *   2. Пропишите KV id в wrangler-sc.toml
 *   3. Задайте секреты в Cloudflare:
 *      npx wrangler secret put SC_CLIENT_ID
 *      npx wrangler secret put SC_CLIENT_SECRET
 *   4. Деплой: npx wrangler deploy -c wrangler-sc.toml
 *
 * В SoundCloud App укажите Redirect URI:
 *   https://<ваш-воркер>.workers.dev/callback
 */

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const origin = url.origin;

    // 1. Старт OAuth из системного браузера
    if (url.pathname === '/auth/start') {
      const sessionId = (url.searchParams.get('session_id') || '').trim();
      const codeChallenge = (url.searchParams.get('code_challenge') || '').trim();
      if (!sessionId || !codeChallenge) {
        return new Response('Missing session_id or code_challenge', { status: 400 });
      }
      if (!env.SC_CLIENT_ID) {
        return new Response('SC_CLIENT_ID is not configured on Worker', { status: 500 });
      }

      await env.SC_SESSIONS.put(
        `sc:${sessionId}`,
        JSON.stringify({ status: 'pending' }),
        { expirationTtl: 180 }
      );

      const authUrl = new URL('https://secure.soundcloud.com/authorize');
      authUrl.searchParams.set('client_id', env.SC_CLIENT_ID);
      authUrl.searchParams.set('redirect_uri', `${origin}/callback`);
      authUrl.searchParams.set('response_type', 'code');
      authUrl.searchParams.set('code_challenge', codeChallenge);
      authUrl.searchParams.set('code_challenge_method', 'S256');
      authUrl.searchParams.set('state', sessionId);

      return Response.redirect(authUrl.toString(), 302);
    }

    // 2. Callback от SoundCloud с authorization code
    if (url.pathname === '/callback') {
      const code = (url.searchParams.get('code') || '').trim();
      const sessionId = (url.searchParams.get('state') || '').trim();
      const err = url.searchParams.get('error');

      if (err && sessionId) {
        await env.SC_SESSIONS.put(
          `sc:${sessionId}`,
          JSON.stringify({ status: 'error', error: err }),
          { expirationTtl: 180 }
        );
        return htmlPage('Ошибка авторизации SoundCloud', `Код ошибки: ${err}. Вернитесь в Flowly.`);
      }

      if (!code || !sessionId) {
        return htmlPage('Неверные параметры', 'Не получен code или state. Попробуйте войти снова.');
      }

      await env.SC_SESSIONS.put(
        `sc:${sessionId}`,
        JSON.stringify({ status: 'code_ready', code }),
        { expirationTtl: 180 }
      );

      return htmlPage(
        'SoundCloud подключён!',
        'Сессия передана в Flowly. Эту вкладку можно закрыть.'
      );
    }

    // 3. Опрос из Rust каждые 700 мс (обмен code + code_verifier на access_token)
    if (url.pathname === '/session') {
      const sessionId = (url.searchParams.get('session_id') || '').trim();
      const codeVerifier = (url.searchParams.get('code_verifier') || '').trim();
      if (!sessionId) {
        return json({ status: 'error', error: 'Missing session_id' }, 400);
      }

      const raw = await env.SC_SESSIONS.get(`sc:${sessionId}`);
      if (!raw) {
        return json({ status: 'pending' });
      }

      const data = JSON.parse(raw);
      if (data.status === 'pending' || data.status === 'exchanging') {
        return json({ status: 'pending' });
      }
      if (data.status === 'ok' && data.access_token) {
        await env.SC_SESSIONS.delete(`sc:${sessionId}`);
        return json({ status: 'ok', access_token: data.access_token });
      }
      if (data.status === 'error') {
        await env.SC_SESSIONS.delete(`sc:${sessionId}`);
        return json({ status: 'error', error: data.error || 'OAuth error' });
      }
      if (data.status === 'code_ready' && data.code) {
        if (!codeVerifier) {
          return json({ status: 'error', error: 'Missing code_verifier' }, 400);
        }

        // Блокируем повторный обмен того же code при параллельном опросе
        await env.SC_SESSIONS.put(
          `sc:${sessionId}`,
          JSON.stringify({ status: 'exchanging' }),
          { expirationTtl: 60 }
        );

        const body = new URLSearchParams({
          grant_type: 'authorization_code',
          client_id: env.SC_CLIENT_ID || '',
          client_secret: env.SC_CLIENT_SECRET || '',
          redirect_uri: `${origin}/callback`,
          code_verifier: codeVerifier,
          code: data.code,
        });

        const tokenResp = await fetch('https://secure.soundcloud.com/oauth/token', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/x-www-form-urlencoded',
            Accept: 'application/json',
          },
          body: body.toString(),
        });

        if (!tokenResp.ok) {
          const txt = await tokenResp.text().catch(() => '');
          await env.SC_SESSIONS.delete(`sc:${sessionId}`);
          return json({
            status: 'error',
            error: `SoundCloud token exchange failed (${tokenResp.status}): ${txt.slice(0, 140)}`,
          });
        }

        const tokJson = await tokenResp.json();
        if (!tokJson.access_token) {
          await env.SC_SESSIONS.delete(`sc:${sessionId}`);
          return json({ status: 'error', error: 'No access_token in SoundCloud response' });
        }

        let username = null;
        try {
          const meResp = await fetch('https://api.soundcloud.com/me', {
            headers: {
              Authorization: `Bearer ${tokJson.access_token}`,
              Accept: 'application/json',
            },
          });
          if (meResp.ok) {
            const me = await meResp.json();
            username = me?.username || me?.full_name || null;
          }
        } catch {
          // fallback to Rust verification
        }

        await env.SC_SESSIONS.delete(`sc:${sessionId}`);
        return json({ status: 'ok', access_token: tokJson.access_token, username });
      }

      return json({ status: 'pending' });
    }

    // 4. Прокси для SoundCloud API и аудиопотоков (работа без VPN/прокси на клиенте)
    if (url.pathname === '/sc-proxy') {
      const targetRaw = (url.searchParams.get('url') || '').trim();
      if (!targetRaw) {
        return new Response('Missing url', { status: 400 });
      }
      let targetUrl;
      try {
        targetUrl = new URL(targetRaw);
      } catch {
        return new Response('Invalid url', { status: 400 });
      }
      const host = targetUrl.hostname.toLowerCase();
      const allowed =
        host === 'api.soundcloud.com' ||
        host === 'api-v2.soundcloud.com' ||
        host === 'soundcloud.com' ||
        host === 'www.soundcloud.com' ||
        host === 'sndcdn.com' ||
        host.endsWith('.sndcdn.com');
      if (!allowed) {
        return new Response('Forbidden host', { status: 403 });
      }

      const fwdHeaders = new Headers();
      for (const h of ['authorization', 'accept', 'range', 'content-type', 'user-agent']) {
        const v = request.headers.get(h);
        if (v) fwdHeaders.set(h, v);
      }
      if (!fwdHeaders.has('user-agent')) {
        fwdHeaders.set(
          'user-agent',
          'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36'
        );
      }

      const upstream = await fetch(targetUrl.toString(), {
        method: request.method,
        headers: fwdHeaders,
        body: request.method !== 'GET' && request.method !== 'HEAD' ? request.body : undefined,
        redirect: 'follow',
      });

      const respHeaders = new Headers();
      for (const h of [
        'content-type',
        'content-length',
        'content-range',
        'accept-ranges',
        'cache-control',
      ]) {
        const v = upstream.headers.get(h);
        if (v) respHeaders.set(h, v);
      }
      respHeaders.set('access-control-allow-origin', '*');

      return new Response(upstream.body, {
        status: upstream.status,
        headers: respHeaders,
      });
    }

    return new Response('Flowly SoundCloud OAuth Worker', { status: 200 });
  },
};

function json(obj, status = 200) {
  return new Response(JSON.stringify(obj), {
    status,
    headers: { 'Content-Type': 'application/json; charset=utf-8' },
  });
}

function htmlPage(title, desc) {
  const html = `<!doctype html>
<html lang="ru"><head><meta charset="utf-8"><title>${title}</title>
<style>body{font-family:system-ui,sans-serif;background:#0e0e13;color:#f4f4f6;display:grid;place-items:center;min-height:100vh;margin:0}
.card{background:#181820;border:1px solid rgba(255,255,255,.1);border-radius:16px;padding:28px 32px;max-width:420px;text-align:center}
h2{margin:0 0 10px;font-size:18px}p{margin:0;color:#a1a1aa;font-size:14px;line-height:1.5}</style></head>
<body><div class="card"><h2>${title}</h2><p>${desc}</p></div></body></html>`;
  return new Response(html, {
    status: 200,
    headers: { 'Content-Type': 'text/html; charset=utf-8' },
  });
}
