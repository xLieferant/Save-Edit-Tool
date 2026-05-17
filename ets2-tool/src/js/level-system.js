const LEVEL_TABLE_URL = new URL("../data/level-table.json", import.meta.url);

let levelTablePromise = null;

function asFiniteNumber(value, fallback = 0) {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : fallback;
}

function normalizeTable(entries) {
  if (!Array.isArray(entries) || !entries.length) return [];

  return entries
    .map((entry) => ({
      level: Math.max(0, Math.floor(asFiniteNumber(entry?.level, 0))),
      increase: Math.max(0, Math.floor(asFiniteNumber(entry?.increase, 0))),
      total_xp: Math.max(0, Math.floor(asFiniteNumber(entry?.total_xp, 0))),
    }))
    .sort((left, right) => left.level - right.level);
}

export async function loadLevelTable() {
  if (!levelTablePromise) {
    levelTablePromise = fetch(LEVEL_TABLE_URL)
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`Failed to load level table: ${response.status}`);
        }
        return normalizeTable(await response.json());
      })
      .catch((error) => {
        console.error("[level-system] level table load failed", error);
        return [];
      });
  }

  return levelTablePromise;
}

export function getMaxLevel(table = []) {
  if (!table.length) return 0;
  return table[table.length - 1].level;
}

export function clampLevel(level, table = []) {
  const maxLevel = getMaxLevel(table);
  const normalized = Math.floor(asFiniteNumber(level, 0));
  return Math.min(Math.max(normalized, 0), maxLevel);
}

export function getLevelEntry(level, table = []) {
  if (!table.length) return null;
  const targetLevel = clampLevel(level, table);
  return table.find((entry) => entry.level === targetLevel) || table[0] || null;
}

export function getXpForLevel(level, table = []) {
  return getLevelEntry(level, table)?.total_xp ?? 0;
}

export function clampXp(xp, table = []) {
  const normalized = Math.max(0, Math.floor(asFiniteNumber(xp, 0)));
  if (!table.length) return normalized;
  return Math.min(normalized, getXpForLevel(getMaxLevel(table), table));
}

export function getLevelForXp(xp, table = []) {
  if (!table.length) return 0;

  const clampedXp = clampXp(xp, table);
  let resolvedLevel = table[0].level;

  for (const entry of table) {
    if (entry.total_xp > clampedXp) break;
    resolvedLevel = entry.level;
  }

  return resolvedLevel;
}

export function getNextLevelEntry(level, table = []) {
  if (!table.length) return null;
  const normalizedLevel = clampLevel(level, table);
  return table.find((entry) => entry.level === normalizedLevel + 1) || null;
}

export function getLevelIncrease(level, table = []) {
  const entry = getLevelEntry(level, table);
  if (!entry) return 0;
  const nextEntry = getNextLevelEntry(level, table);
  return nextEntry ? nextEntry.total_xp - entry.total_xp : 0;
}

export function getLevelProgress(xp, table = []) {
  const clampedXp = clampXp(xp, table);
  const level = getLevelForXp(clampedXp, table);
  const currentEntry = getLevelEntry(level, table);
  const nextEntry = getNextLevelEntry(level, table);
  const currentLevelXp = currentEntry?.total_xp ?? 0;
  const nextLevelXp = nextEntry?.total_xp ?? currentLevelXp;
  const xpIntoLevel = Math.max(0, clampedXp - currentLevelXp);
  const xpNeededForNextLevel = Math.max(0, nextLevelXp - currentLevelXp);
  const progressRatio = xpNeededForNextLevel > 0 ? Math.min(xpIntoLevel / xpNeededForNextLevel, 1) : 1;

  return {
    level,
    currentXp: clampedXp,
    currentLevelXp,
    nextLevelXp,
    xpIntoLevel,
    xpNeededForNextLevel,
    xpRemaining: Math.max(0, nextLevelXp - clampedXp),
    progressRatio,
    progressPercent: Math.round(progressRatio * 100),
    isMaxLevel: !nextEntry,
  };
}
