export interface CoverPalette {
  primary: string;
  secondary: string;
  tertiary: string;
  quaternary: string;
  quinary: string;
}

function colorDist(a: [number, number, number], b: [number, number, number]): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

function rgbToHex(r: number, g: number, b: number): string {
  return (
    '#' +
    [r, g, b]
      .map((v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, '0'))
      .join('')
  );
}

function saturation(r: number, g: number, b: number): number {
  const max = Math.max(r, g, b) / 255;
  const min = Math.min(r, g, b) / 255;
  if (max === min) return 0;
  const l = (max + min) / 2;
  return (max - min) / (1 - Math.abs(2 * l - 1));
}

function luminance(r: number, g: number, b: number): number {
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255;
}

function boostVibrant(r: number, g: number, b: number): [number, number, number] {
  const lum = luminance(r, g, b);
  if (lum < 0.28) {
    const factor = 0.36 / Math.max(0.08, lum);
    return [
      Math.min(255, r * factor),
      Math.min(255, g * factor),
      Math.min(255, b * factor),
    ];
  }
  return [r, g, b];
}

export async function extractCoverPalette(
  artworkUrl: string,
  fallbackHex: string,
  proxyPort: number
): Promise<CoverPalette> {
  const fallback: CoverPalette = {
    primary: fallbackHex,
    secondary: '#a855f7',
    tertiary: '#3b82f6',
    quaternary: '#ec4899',
    quinary: '#10b981',
  };

  if (!artworkUrl) return fallback;

  const src =
    proxyPort > 0 && artworkUrl.startsWith('http')
      ? `http://127.0.0.1:${proxyPort}/proxy?source=image&url=${encodeURIComponent(artworkUrl)}`
      : artworkUrl;

  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = 'anonymous';

    img.onload = () => {
      try {
        const canvas = document.createElement('canvas');
        const size = 24;
        canvas.width = size;
        canvas.height = size;
        const ctx = canvas.getContext('2d');
        if (!ctx) return resolve(fallback);

        ctx.drawImage(img, 0, 0, size, size);
        const data = ctx.getImageData(0, 0, size, size).data;

        const buckets = new Map<string, { r: number; g: number; b: number; score: number; count: number }>();

        for (let i = 0; i < data.length; i += 4) {
          const r = data[i];
          const g = data[i + 1];
          const b = data[i + 2];
          const a = data[i + 3];
          if (a < 128) continue;

          const lum = luminance(r, g, b);
          if (lum < 0.08 || lum > 0.92) continue;

          const sat = saturation(r, g, b);
          const qr = Math.round(r / 32) * 32;
          const qg = Math.round(g / 32) * 32;
          const qb = Math.round(b / 32) * 32;
          const key = `${qr},${qg},${qb}`;

          const weight = 1 + sat * 4.5;
          const cur = buckets.get(key);
          if (cur) {
            cur.r += r;
            cur.g += g;
            cur.b += b;
            cur.count += 1;
            cur.score += weight;
          } else {
            buckets.set(key, { r, g, b, count: 1, score: weight });
          }
        }

        const sortedRgb = [...buckets.values()]
          .sort((a, b) => b.score - a.score)
          .map((item) =>
            boostVibrant(item.r / item.count, item.g / item.count, item.b / item.count)
          );

        if (sortedRgb.length === 0) return resolve(fallback);

        // Выбираем до 5 максимально различных оттенков для богатого Ambient Light
        const picked: [number, number, number][] = [];
        for (const rgb of sortedRgb) {
          if (picked.every((p) => colorDist(p, rgb) >= 44)) {
            picked.push(rgb);
            if (picked.length >= 5) break;
          }
        }
        for (const rgb of sortedRgb) {
          if (picked.length >= 5) break;
          if (!picked.includes(rgb)) picked.push(rgb);
        }

        const hexes = picked.map(([r, g, b]) => rgbToHex(r, g, b));
        resolve({
          primary: hexes[0],
          secondary: hexes[1] ?? hexes[0],
          tertiary: hexes[2] ?? hexes[1] ?? hexes[0],
          quaternary: hexes[3] ?? hexes[2] ?? hexes[0],
          quinary: hexes[4] ?? hexes[1] ?? hexes[0],
        });
      } catch {
        resolve(fallback);
      }
    };

    img.onerror = () => resolve(fallback);
    img.src = src;
  });
}
