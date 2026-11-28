import type { Track } from './types';
import type { UserRole } from './supabase';

export interface JamMember {
  id: string;
  name: string;
  avatarUrl?: string;
  tag?: string;
  role?: UserRole;
  isHost: boolean;
  tasteLabel: string;
  trackCount: number;
  tracks: Track[];
}

export interface BlendedTrack {
  track: Track;
  reason: string;
  matchCount: number;
  score: number;
}

export interface JamChatMessage {
  id: string;
  senderId: string;
  senderName: string;
  text: string;
  ts: number;
}

export interface PublicJamRoomInfo {
  roomCode: string;
  title: string;
  hostName: string;
  memberCount: number;
  currentTrackTitle?: string;
  currentTrackArtist?: string;
  currentTrackArtwork?: string;
  isPrivate: boolean;
  updatedAt: number;
}

export interface JamRoomState {
  roomCode: string;
  myId: string;
  isHost: boolean;
  connected: boolean;
  isPrivate?: boolean;
  roomTitle?: string;
  members: JamMember[];
  queue: BlendedTrack[];
  currentTrack: Track | null;
  isPlaying: boolean;
  positionSec: number;
  skipVotes: string[];
  statusText: string;
  syncStatus?: 'synced' | 'catching-up' | 'host-away';
  chat?: JamChatMessage[];
}

export function pickDiverseSample(tracks: Track[], count: number): Track[] {
  if (tracks.length <= count) return [...tracks];
  const copy = [...tracks];
  for (let i = copy.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [copy[i], copy[j]] = [copy[j], copy[i]];
  }
  return copy.slice(0, count);
}

function primaryArtist(artist: string): string {
  return (artist.split(/[,&]/)[0] ?? artist).trim().toLowerCase();
}

function normalizeTitle(title: string): string {
  return title
    .toLowerCase()
    .replace(/\(.*?\)|\[.*?\]/g, '')
    .replace(/[^a-zа-яё0-9\s]/gi, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

export function trackMatchKey(t: Track): string {
  return `${primaryArtist(t.artist)}::${normalizeTitle(t.title)}`;
}

/**
 * Алгоритм совмещения вкусов участников комнаты:
 * 1) Общие совпадения треков (>= 2 участников)
 * 2) Общие артисты (>= 2 участников)
 * 3) Общие жанры + честный Round-Robin между всеми участниками
 */
export function buildBlendedQueue(
  members: JamMember[],
  maxTracks = 120,
  excludeKeys?: Set<string>,
  shuffleJitter = false
): BlendedTrack[] {
  if (members.length === 0) return [];

  const trackOwners = new Map<string, { track: Track; owners: Set<string>; ownerNames: string[] }>();
  const artistOwners = new Map<string, Set<string>>();
  const genreCounts = new Map<string, number>();

  for (const m of members) {
    const seenMemberKeys = new Set<string>();
    for (const tr of m.tracks) {
      const key = trackMatchKey(tr);
      if (seenMemberKeys.has(key)) continue;
      seenMemberKeys.add(key);

      const art = primaryArtist(tr.artist);
      if (!artistOwners.has(art)) artistOwners.set(art, new Set());
      artistOwners.get(art)!.add(m.id);

      if (tr.genre) {
        const g = tr.genre.trim().toLowerCase();
        if (g) genreCounts.set(g, (genreCounts.get(g) ?? 0) + 1);
      }

      const existing = trackOwners.get(key);
      if (existing) {
        existing.owners.add(m.id);
        if (!existing.ownerNames.includes(m.name)) {
          existing.ownerNames.push(m.name);
        }
      } else {
        trackOwners.set(key, {
          track: tr,
          owners: new Set([m.id]),
          ownerNames: [m.name],
        });
      }
    }
  }

  const sharedMatches: BlendedTrack[] = [];
  const sharedArtists: BlendedTrack[] = [];
  const byMember = new Map<string, BlendedTrack[]>();
  for (const m of members) byMember.set(m.id, []);

  for (const [key, info] of trackOwners.entries()) {
    const matchCount = info.owners.size;
    const art = primaryArtist(info.track.artist);
    const artMatch = artistOwners.get(art)?.size ?? 1;
    const g = info.track.genre?.trim().toLowerCase() ?? '';
    const genreBoost = g ? (genreCounts.get(g) ?? 0) : 0;

    const jitter = shuffleJitter ? Math.random() * 25 : 0;

    if (matchCount >= 2) {
      const namesPreview = info.ownerNames.slice(0, 2).join(' и ');
      const extra = info.ownerNames.length > 2 ? ` +${info.ownerNames.length - 2}` : '';
      sharedMatches.push({
        track: info.track,
        reason: `Совпадение: ${namesPreview}${extra}`,
        matchCount,
        score: matchCount * 100 + genreBoost + jitter,
      });
    } else if (artMatch >= 2) {
      sharedArtists.push({
        track: info.track,
        reason: `Общий артист · ${info.ownerNames[0]}`,
        matchCount: 1,
        score: artMatch * 40 + genreBoost + jitter,
      });
    } else {
      const firstOwnerId = [...info.owners][0];
      const list = byMember.get(firstOwnerId);
      if (list) {
        list.push({
          track: info.track,
          reason: `Из вкусов: ${info.ownerNames[0]}`,
          matchCount: 1,
          score: genreBoost * 10 + Math.random() * (shuffleJitter ? 30 : 5),
        });
      }
    }
    void key;
  }

  sharedMatches.sort((a, b) => b.score - a.score);
  sharedArtists.sort((a, b) => b.score - a.score);

  // Сортируем индивидуальные треки по популярности жанра в комнате + лёгкий рандом
  for (const list of byMember.values()) {
    list.sort((a, b) => b.score - a.score);
  }

  // Round-Robin перемешивание индивидуальных треков
  const roundRobin: BlendedTrack[] = [];
  const memberLists = [...byMember.values()];
  let idx = 0;
  while (roundRobin.length < maxTracks) {
    let added = false;
    for (const list of memberLists) {
      if (idx < list.length) {
        roundRobin.push(list[idx]);
        added = true;
      }
    }
    if (!added) break;
    idx += 1;
  }

  const combined = [...sharedMatches, ...sharedArtists, ...roundRobin];
  const deduped: BlendedTrack[] = [];
  const seen = new Set<string>();
  for (const item of combined) {
    const key = trackMatchKey(item.track);
    if (seen.has(key) || excludeKeys?.has(key)) continue;
    seen.add(key);
    deduped.push(item);
    if (deduped.length >= maxTracks) break;
  }

  return deduped;
}

export function generateRoomCode(): string {
  const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
  let code = '';
  for (let i = 0; i < 4; i++) {
    code += chars[Math.floor(Math.random() * chars.length)];
  }
  return `FLOW-${code}`;
}
