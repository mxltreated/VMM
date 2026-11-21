function normalizeText(input: string): string {
  return input
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();
}

function compactText(input: string): string {
  return input.toLowerCase().replace(/[^a-z0-9]+/g, '');
}

function tokenize(input: string): string[] {
  return input
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replace(/([a-zA-Z])([0-9])/g, '$1 $2')
    .replace(/([0-9])([a-zA-Z])/g, '$1 $2')
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(Boolean);
}

function maxEditDistance(len: number): number {
  if (len <= 3) return 0;
  if (len <= 6) return 1;
  return 2;
}

function boundedDamerauLevenshtein(
  a: string,
  b: string,
  maxDist: number
): number | null {
  const m = a.length;
  const n = b.length;
  if (Math.abs(m - n) > maxDist) return null;
  if (maxDist === 0) return a === b ? 0 : null;
  if (m === 0) return n;
  if (n === 0) return m;

  const inf = maxDist + 1;
  const cols = n + 1;
  const dp = new Array<number>((m + 1) * cols).fill(inf);

  for (let i = 0; i <= m; i++) dp[i * cols] = i;
  for (let j = 0; j <= n; j++) dp[j] = j;

  for (let i = 1; i <= m; i++) {
    let rowMin = inf;
    for (let j = 1; j <= n; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      let best = Math.min(
        dp[(i - 1) * cols + j] + 1,
        dp[i * cols + (j - 1)] + 1,
        dp[(i - 1) * cols + (j - 1)] + cost
      );
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) {
        best = Math.min(best, dp[(i - 2) * cols + (j - 2)] + 1);
      }
      dp[i * cols + j] = best;
      if (best < rowMin) rowMin = best;
    }
    if (rowMin > maxDist) return null;
  }

  const dist = dp[m * cols + n];
  return dist <= maxDist ? dist : null;
}

export interface SearchField {
  text: string;
  weight: number;
  allowFuzzy?: boolean;
}

export function scoreSearchFields(query: string, fields: SearchField[]): number {
  const qNorm = normalizeText(query);
  const qCompact = compactText(query);
  const qTokens = tokenize(query);

  if (!qCompact && qTokens.length === 0) return 1;

  let bestScore = 0;

  for (const field of fields) {
    if (!field.text) continue;
    const norm = normalizeText(field.text);
    const compact = compactText(field.text);
    let fieldScore = 0;

    if (norm === qNorm) {
      fieldScore = 100;
    } else if (compact === qCompact) {
      fieldScore = 95;
    } else if (qNorm && norm.startsWith(qNorm)) {
      fieldScore = 85;
    } else if (qCompact && compact.startsWith(qCompact)) {
      fieldScore = 80;
    } else if (qNorm && norm.includes(qNorm)) {
      fieldScore = 70;
    } else if (qCompact && compact.includes(qCompact)) {
      fieldScore = 65;
    } else {
      const targetTokens = tokenize(field.text);
      if (targetTokens.length > 0 && qTokens.length > 0) {
        let penalty = 0;
        let allMatched = true;
        const matchedIndices: number[] = [];

        for (const qt of qTokens) {
          let bestIdx = -1;
          let bestPenalty = Infinity;

          for (let i = 0; i < targetTokens.length; i++) {
            const tt = targetTokens[i];
            if (tt === qt) {
              bestIdx = i;
              bestPenalty = 0;
              break;
            }
            if (qt.length >= 2 && tt.startsWith(qt)) {
              if (1 < bestPenalty) {
                bestIdx = i;
                bestPenalty = 1;
              }
              continue;
            }
            if (field.allowFuzzy !== false) {
              const maxD = maxEditDistance(qt.length);
              if (maxD > 0) {
                const dist = boundedDamerauLevenshtein(qt, tt, maxD);
                if (dist !== null && dist * 10 < bestPenalty) {
                  bestIdx = i;
                  bestPenalty = dist * 10;
                }
              }
            }
          }

          if (bestIdx === -1) {
            allMatched = false;
            break;
          }
          penalty += bestPenalty;
          matchedIndices.push(bestIdx);
        }

        if (allMatched) {
          let inOrder = true;
          for (let i = 1; i < matchedIndices.length; i++) {
            if (matchedIndices[i] <= matchedIndices[i - 1]) {
              inOrder = false;
              break;
            }
          }
          fieldScore = Math.max(10, 55 - penalty + (inOrder ? 5 : 0));
        } else if (
          field.allowFuzzy !== false &&
          qTokens.length === 1 &&
          qCompact.length >= 6 &&
          targetTokens.length >= 2
        ) {
          const maxD = qCompact.length >= 10 ? 2 : 1;
          let bestDist: number | null = null;
          for (let w = 2; w <= Math.min(3, targetTokens.length); w++) {
            for (let i = 0; i <= targetTokens.length - w; i++) {
              const joined = targetTokens.slice(i, i + w).join('');
              const dist = boundedDamerauLevenshtein(qCompact, joined, maxD);
              if (dist !== null && (bestDist === null || dist < bestDist)) {
                bestDist = dist;
              }
            }
          }
          if (bestDist !== null) {
            fieldScore = Math.max(10, 42 - bestDist * 6);
          }
        }
      }
    }

    if (fieldScore > 0) {
      bestScore = Math.max(bestScore, fieldScore * field.weight);
    }
  }

  return bestScore;
}
