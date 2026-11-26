import type { Track } from './types';

const WORD_HINTS: Record<string, string> = {
  дефт: 'daft',
  дафт: 'daft',
  панк: 'punk',
  уикенд: 'weeknd',
  викенд: 'weeknd',
  кендрик: 'kendrick',
  ламар: 'lamar',
  канье: 'kanye',
  канни: 'kanye',
  уэст: 'west',
  вест: 'west',
  дрейк: 'drake',
  драке: 'drake',
  трэвис: 'travis',
  тревис: 'travis',
  скотт: 'scott',
  плейбой: 'playboi',
  карти: 'carti',
  билли: 'billie',
  айлиш: 'eilish',
  тейлор: 'taylor',
  свифт: 'swift',
  линкин: 'linkin',
  парк: 'park',
  рамштайн: 'rammstein',
  металлика: 'metallica',
  металика: 'metallica',
  нирвана: 'nirvana',
  радиохед: 'radiohead',
  пост: 'post',
  малон: 'malone',
  малоун: 'malone',
  лил: 'lil',
  пип: 'peep',
  джус: 'juice',
  ворлд: 'world',
  фонк: 'phonk',
  рэп: 'rap',
  рок: 'rock',
};

const RU_TO_EN: Record<string, string> = {
  а: 'a',
  б: 'b',
  в: 'v',
  г: 'g',
  д: 'd',
  е: 'e',
  ё: 'yo',
  ж: 'zh',
  з: 'z',
  и: 'i',
  й: 'y',
  к: 'k',
  л: 'l',
  м: 'm',
  н: 'n',
  о: 'o',
  п: 'p',
  р: 'r',
  с: 's',
  т: 't',
  у: 'u',
  ф: 'f',
  х: 'kh',
  ц: 'ts',
  ч: 'ch',
  ш: 'sh',
  щ: 'shch',
  ъ: '',
  ы: 'y',
  ь: '',
  э: 'e',
  ю: 'yu',
  я: 'ya',
};

export function hasCyrillic(text: string): boolean {
  return /[а-яё]/i.test(text);
}

export function transliterateRuToEn(text: string): string {
  const words = text.toLowerCase().trim().split(/\s+/);
  return words
    .map((w) => {
      if (WORD_HINTS[w]) return WORD_HINTS[w];
      let out = '';
      for (const ch of w) {
        out += RU_TO_EN[ch] ?? ch;
      }
      return out;
    })
    .join(' ');
}

function normalizeStr(s: string): string {
  return s
    .toLowerCase()
    .replace(/ё/g, 'е')
    .replace(/[^a-zа-я0-9\s]/gi, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

export function levenshtein(a: string, b: string): number {
  const m = a.length;
  const n = b.length;
  if (m === 0) return n;
  if (n === 0) return m;

  const dp = new Array<number>(n + 1);
  for (let j = 0; j <= n; j++) dp[j] = j;

  for (let i = 1; i <= m; i++) {
    let prev = dp[0];
    dp[0] = i;
    for (let j = 1; j <= n; j++) {
      const temp = dp[j];
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      dp[j] = Math.min(dp[j] + 1, dp[j - 1] + 1, prev + cost);
      prev = temp;
    }
  }
  return dp[n];
}

function tokenSimilarity(q: string, t: string): number {
  if (!q || !t) return 0;
  if (t.includes(q)) return q.length === t.length ? 1 : 0.94;
  if (q.length >= 3 && q.includes(t) && t.length >= 3) return 0.85;
  const dist = levenshtein(q, t);
  const maxLen = Math.max(q.length, t.length);
  return Math.max(0, 1 - dist / maxLen);
}

export function fuzzyTrackScore(query: string, track: Track): number {
  const qNorm = normalizeStr(query);
  if (!qNorm) return 0;

  const targetFull = normalizeStr(`${track.artist} ${track.title} ${track.album}`);
  const qTranslit = hasCyrillic(qNorm) ? normalizeStr(transliterateRuToEn(qNorm)) : qNorm;

  if (targetFull.includes(qNorm) || targetFull.includes(qTranslit)) {
    return 1.0;
  }

  const targetTokens = targetFull.split(' ').filter(Boolean);
  const qTokensOrig = qNorm.split(' ').filter(Boolean);
  const qTokensTrans = qTranslit.split(' ').filter(Boolean);

  const scoreTokens = (qToks: string[]) => {
    if (qToks.length === 0) return 0;
    let sum = 0;
    for (const qt of qToks) {
      let best = 0;
      for (const tt of targetTokens) {
        const s = tokenSimilarity(qt, tt);
        if (s > best) best = s;
        if (best >= 0.98) break;
      }
      if (best < 0.52) return 0;
      sum += best;
    }
    return sum / qToks.length;
  };

  return Math.max(scoreTokens(qTokensOrig), scoreTokens(qTokensTrans));
}

export function fuzzySearchTracks(query: string, tracks: Track[], limit = 30): Track[] {
  const trimmed = query.trim();
  if (!trimmed) return tracks;

  const scored: { track: Track; score: number }[] = [];
  const seen = new Set<string>();
  for (const t of tracks) {
    if (seen.has(t.id)) continue;
    const score = fuzzyTrackScore(trimmed, t);
    if (score >= 0.55) {
      seen.add(t.id);
      scored.push({ track: t, score });
    }
  }
  scored.sort((a, b) => b.score - a.score);
  return scored.slice(0, limit).map((s) => s.track);
}
