// Синхронизация Jam-комнаты: порт логики Together из yandex-music-client
// (host-authoritative state + ready-gating + drift correction).
// Транспорт наш — Supabase Realtime / Relay, протокол сообщений их.

import type { Track } from './types';

export const DRIFT_LIMIT = 1.2; // сек: расхождение позиции, при котором гость делает seek
export const HEARTBEAT_MS = 1000; // тик синхронизации
export const RESEND_MS = 5000; // полный пуш состояния раз в 5 секунд
export const PUSH_DEBOUNCE = 80; // дебаунс пушей при локальных изменениях
export const QUEUE_LIMIT = 120;
export const SYNC_BUDGET = 40 * 1024; // байт: потолок payload с очередью

/** Полное состояние хоста: трек + позиция + очередь (или лёгкий пульс без неё) */
export interface JamStatePayload {
  kind: 'state';
  trackId: string | null;
  positionMs: number;
  paused: boolean;
  updatedAt: number;
  title: string | null;
  track: Track | null;
  queue: Track[];
  index: number;
}

/** Гость сообщает хосту, готов ли его плеер к воспроизведению */
export interface JamReadyPayload {
  kind: 'ready';
  trackId: string | null;
  ready: boolean;
}

/** Заметка в чат */
export interface JamNotePayload {
  kind: 'note';
  text: string;
}

export type JamSyncPayload = JamStatePayload | JamReadyPayload | JamNotePayload;

export interface JamPlayerSnapshot {
  currentTrack: Track | null;
  queue: Track[];
  positionSec: number;
  isPlaying: boolean;
}

/** Собрать состояние хоста: с очередью (полный пуш) или лёгкий пульс без неё */
export function buildState(host: JamPlayerSnapshot, withQueue = true): JamStatePayload {
  const trackId = host.currentTrack?.id ?? null;
  const queueTracks = host.queue;
  const index = trackId ? queueTracks.findIndex((t) => t.id === trackId) : -1;

  const base: JamStatePayload = {
    kind: 'state',
    trackId,
    positionMs: Math.round(host.positionSec * 1000),
    paused: !host.isPlaying,
    updatedAt: Date.now(),
    title: host.currentTrack?.title ?? null,
    track: host.currentTrack,
    queue: [],
    index: Math.max(0, index),
  };

  if (!withQueue) return base;

  // Бюджет payload: обрезаем очередь вокруг текущего трека, пока не влезает
  let list = queueTracks.slice(0, QUEUE_LIMIT);
  let idx = Math.max(0, index);
  let payload: JamStatePayload = { ...base, queue: list, index: idx };

  let guard = 0;
  while (JSON.stringify(payload).length > SYNC_BUDGET && guard < 40) {
    guard += 1;
    if (list.length > idx + 6) {
      list = list.slice(0, list.length - 8); // режем хвост
    } else if (idx > 6) {
      list = list.slice(8); // режем начало
      idx = Math.max(0, idx - 8);
    } else {
      break;
    }
    payload = { ...base, queue: list, index: idx };
  }

  return payload;
}

/** Ожидаемая позиция гостя: позиция хоста + лаг с момента отправки */
export function expectedPosition(payload: JamStatePayload): number {
  const lag = payload.paused ? 0 : Math.max(0, Date.now() - payload.updatedAt) / 1000;
  return payload.positionMs / 1000 + lag;
}

/** Найти стартовый индекс в очереди хоста: по index, затем по trackId */
export function findStartIndex(queue: Track[], index: number, trackId: string | null): number {
  if (index >= 0 && index < queue.length) return index;
  if (trackId) {
    const at = queue.findIndex((t) => t.id === trackId);
    if (at >= 0) return at;
  }
  return 0;
}
