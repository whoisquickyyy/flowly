// Вставьте сюда данные вашего проекта Supabase (Settings -> API)
export const SUPABASE_URL = 'https://vgdypuwsofnunubthwhr.supabase.co';
export const SUPABASE_ANON_KEY = 'sb_publishable_bfpURwyVAqTujQvn2X7kyg_y0QlPxGC';

export type UserRole = 'creator' | 'admin' | 'vip' | 'beta' | 'listener';

export const ROLE_BADGES: Record<
  UserRole,
  { label: string; color: string; bg: string } | null
> = {
  creator: { label: 'Создатель', color: '#fbbf24', bg: 'rgba(251, 191, 36, 0.18)' },
  admin: { label: 'Админ', color: '#f43f5e', bg: 'rgba(244, 63, 94, 0.18)' },
  vip: { label: 'VIP', color: '#2dd4bf', bg: 'rgba(45, 212, 191, 0.18)' },
  beta: { label: 'Тестер', color: '#60a5fa', bg: 'rgba(96, 165, 250, 0.18)' },
  listener: null,
};

export interface FlowlyProfile {
  id: string;
  username: string;
  tag: string;
  avatar_url: string;
  role?: UserRole;
}

export function getSupabaseRealtimeWsUrl(): string {
  const wsBase = SUPABASE_URL.replace(/^https:\/\//i, 'wss://').replace(/^http:\/\//i, 'ws://');
  return `${wsBase}/realtime/v1/websocket?apikey=${encodeURIComponent(SUPABASE_ANON_KEY)}&vsn=1.0.0`;
}

export interface SupabaseSession {
  access_token: string;
  refresh_token: string;
  expires_at: number;
  user: {
    id: string;
    email?: string;
  };
}

const SESSION_KEY = 'flowly_sb_session';
const PROFILE_KEY = 'flowly_sb_profile';

export function isSupabaseConfigured(): boolean {
  return Boolean(SUPABASE_URL.trim() && SUPABASE_ANON_KEY.trim());
}

function baseHeaders(accessToken?: string): Record<string, string> {
  const headers: Record<string, string> = {
    apikey: SUPABASE_ANON_KEY,
    'Content-Type': 'application/json',
  };
  if (accessToken) {
    headers.Authorization = `Bearer ${accessToken}`;
  }
  return headers;
}

function randomTag(): string {
  return Math.floor(1000 + Math.random() * 9000).toString();
}

function saveSession(session: SupabaseSession | null, profile: FlowlyProfile | null) {
  if (typeof localStorage === 'undefined') return;
  if (session) {
    localStorage.setItem(SESSION_KEY, JSON.stringify(session));
  } else {
    localStorage.removeItem(SESSION_KEY);
  }
  if (profile) {
    localStorage.setItem(PROFILE_KEY, JSON.stringify(profile));
  } else {
    localStorage.removeItem(PROFILE_KEY);
  }
}

export function getCachedProfile(): FlowlyProfile | null {
  if (typeof localStorage === 'undefined') return null;
  try {
    const raw = localStorage.getItem(PROFILE_KEY);
    return raw ? (JSON.parse(raw) as FlowlyProfile) : null;
  } catch {
    return null;
  }
}

export async function fetchProfile(userId: string, accessToken: string): Promise<FlowlyProfile | null> {
  if (!isSupabaseConfigured()) return null;
  const resp = await fetch(
    `${SUPABASE_URL}/rest/v1/profiles?id=eq.${encodeURIComponent(userId)}&select=*`,
    { headers: baseHeaders(accessToken) }
  );
  if (!resp.ok) return null;
  const rows = (await resp.json()) as FlowlyProfile[];
  return rows[0] ?? null;
}

export async function signUp(
  email: string,
  password: string,
  username: string
): Promise<{ session: SupabaseSession | null; profile: FlowlyProfile | null; needsEmailConfirm: boolean }> {
  if (!isSupabaseConfigured()) {
    throw new Error('Supabase ещё не настроен (не указаны URL и anon key).');
  }
  const cleanName = username.trim() || 'Слушатель';
  const tag = randomTag();
  const resp = await fetch(`${SUPABASE_URL}/auth/v1/signup`, {
    method: 'POST',
    headers: baseHeaders(),
    body: JSON.stringify({
      email: email.trim(),
      password,
      data: { username: cleanName, tag },
    }),
  });
  const data = await resp.json();
  if (!resp.ok) {
    throw new Error(data.error_description || data.msg || data.message || 'Ошибка регистрации');
  }

  if (!data.access_token || !data.user?.id) {
    return { session: null, profile: null, needsEmailConfirm: true };
  }

  const session: SupabaseSession = {
    access_token: data.access_token,
    refresh_token: data.refresh_token,
    expires_at: Math.floor(Date.now() / 1000) + Number(data.expires_in || 3600),
    user: { id: data.user.id, email: data.user.email },
  };

  // Создаём/обновляем запись в таблице profiles
  const profile: FlowlyProfile = {
    id: data.user.id,
    username: cleanName,
    tag,
    avatar_url: '',
  };
  await fetch(`${SUPABASE_URL}/rest/v1/profiles`, {
    method: 'POST',
    headers: {
      ...baseHeaders(session.access_token),
      Prefer: 'resolution=merge-duplicates',
    },
    body: JSON.stringify(profile),
  }).catch(() => {});

  const fetched = (await fetchProfile(data.user.id, session.access_token)) ?? profile;
  saveSession(session, fetched);
  return { session, profile: fetched, needsEmailConfirm: false };
}

export async function signIn(
  email: string,
  password: string
): Promise<{ session: SupabaseSession; profile: FlowlyProfile }> {
  if (!isSupabaseConfigured()) {
    throw new Error('Supabase ещё не настроен (не указаны URL и anon key).');
  }
  const resp = await fetch(`${SUPABASE_URL}/auth/v1/token?grant_type=password`, {
    method: 'POST',
    headers: baseHeaders(),
    body: JSON.stringify({ email: email.trim(), password }),
  });
  const data = await resp.json();
  if (!resp.ok) {
    throw new Error(data.error_description || data.msg || data.message || 'Неверный email или пароль');
  }

  const session: SupabaseSession = {
    access_token: data.access_token,
    refresh_token: data.refresh_token,
    expires_at: Math.floor(Date.now() / 1000) + Number(data.expires_in || 3600),
    user: { id: data.user.id, email: data.user.email },
  };

  let profile = await fetchProfile(data.user.id, session.access_token);
  if (!profile) {
    const meta = data.user?.user_metadata ?? {};
    profile = {
      id: data.user.id,
      username: meta.username || email.split('@')[0] || 'Слушатель',
      tag: meta.tag || randomTag(),
      avatar_url: meta.avatar_url || '',
    };
    await fetch(`${SUPABASE_URL}/rest/v1/profiles`, {
      method: 'POST',
      headers: {
        ...baseHeaders(session.access_token),
        Prefer: 'resolution=merge-duplicates',
      },
      body: JSON.stringify(profile),
    }).catch(() => {});
  }

  saveSession(session, profile);
  return { session, profile };
}

export async function restoreSession(): Promise<{
  session: SupabaseSession | null;
  profile: FlowlyProfile | null;
}> {
  if (!isSupabaseConfigured() || typeof localStorage === 'undefined') {
    return { session: null, profile: null };
  }
  try {
    const raw = localStorage.getItem(SESSION_KEY);
    if (!raw) return { session: null, profile: null };
    let session = JSON.parse(raw) as SupabaseSession;
    const now = Math.floor(Date.now() / 1000);

    if (session.expires_at - 60 <= now && session.refresh_token) {
      const resp = await fetch(`${SUPABASE_URL}/auth/v1/token?grant_type=refresh_token`, {
        method: 'POST',
        headers: baseHeaders(),
        body: JSON.stringify({ refresh_token: session.refresh_token }),
      });
      if (!resp.ok) {
        saveSession(null, null);
        return { session: null, profile: null };
      }
      const data = await resp.json();
      session = {
        access_token: data.access_token,
        refresh_token: data.refresh_token,
        expires_at: Math.floor(Date.now() / 1000) + Number(data.expires_in || 3600),
        user: { id: data.user.id, email: data.user.email },
      };
    }

    const profile = (await fetchProfile(session.user.id, session.access_token)) ?? getCachedProfile();
    saveSession(session, profile);
    return { session, profile };
  } catch {
    return { session: null, profile: null };
  }
}

export async function updateProfile(
  session: SupabaseSession,
  patch: Partial<Pick<FlowlyProfile, 'username' | 'avatar_url'>>
): Promise<FlowlyProfile> {
  if (!isSupabaseConfigured()) {
    throw new Error('Supabase ещё не настроен.');
  }
  const resp = await fetch(
    `${SUPABASE_URL}/rest/v1/profiles?id=eq.${encodeURIComponent(session.user.id)}`,
    {
      method: 'PATCH',
      headers: {
        ...baseHeaders(session.access_token),
        Prefer: 'return=representation',
      },
      body: JSON.stringify(patch),
    }
  );
  if (!resp.ok) {
    throw new Error('Не удалось обновить профиль');
  }
  const rows = (await resp.json()) as FlowlyProfile[];
  const updated = rows[0];
  if (updated) {
    saveSession(session, updated);
    return updated;
  }
  throw new Error('Профиль не найден');
}

export function signOut() {
  saveSession(null, null);
}
