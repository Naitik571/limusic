// Local personalization: the home Shortcuts grid, sidebar pins, and the play-recency / top-artist
// bookkeeping both of those need. Pure and rune-free on purpose — `personal.check.ts` runs it under
// plain node (`node --experimental-strip-types`). The reactive wrapper and persistence live in
// `player.svelte.ts`; nothing here touches storage, the network, or Svelte.
import type { BrowseItem } from './api';

/**
 * Grid capacity. Everything on the grid was put there by hand, with one exception: `seedPick` lets
 * the app suggest a tile (today only On Repeat, once it has enough songs to be worth one). A seeded
 * tile is never forced: it can't evict a hand-picked one, and removing it is permanent.
 */
export const MAX_PICKS = 18;
export const MAX_PINS = 3;
const MAX_RECENT = 100;
const MAX_ARTISTS = 100;

/**
  * A Shortcuts tile — always something the user added by hand. Array order in `picks` IS display
  * order: the user arranges the grid by dragging, so nothing else may reshuffle it.
  */
export type Pick = BrowseItem & {
	/** Bumped on every play/click — drives eviction only, never order. */
	lastUsedAt: number;
};

export type RecentEntry = BrowseItem & { at: number };

export type Personal = {
	picks: Pick[];
	/** Pinned sidebar playlists, at most MAX_PINS; array order is display order. */
	pins: string[];
	/** Last time each playlist/album/artist was played from, keyed by browseId. */
	recent: Record<string, RecentEntry>;
	/** Play counts per artist, keyed by channel id (or name when there's no id). */
	artists: Record<string, { name: string; count: number }>;
	/**
	 * Tiles the user has taken off the grid. Only `seedPick` consults it, so this is purely "don't
	 * suggest this again": a hand-added tile is unaffected, since `addPick` ignores the list.
	 */
	dismissedSeeds: string[];
};

export function empty(): Personal {
	return { picks: [], pins: [], recent: {}, artists: {}, dismissedSeeds: [] };
}

/** Tolerant parse of a persisted blob — a corrupt or older shape degrades to empty, never throws. */
export function hydrate(raw: unknown): Personal {
	const base = empty();
	if (!raw || typeof raw !== 'object') return base;
	const o = raw as Partial<Personal>;
	if (Array.isArray(o.picks)) {
		// `manual: false` marks a tile from the old auto-seeding build, which filled the grid on the
		// user's behalf. Those are dropped rather than inherited forever. (Today's `seedPick` is a
		// narrower thing: one suggestion, refusable, and it writes no `manual` flag.)
		base.picks = o.picks.filter(
			(p) => p && typeof p.id === 'string' && (p as { manual?: boolean }).manual !== false
		);
	}
	if (Array.isArray(o.pins)) {
		base.pins = o.pins.filter((p) => typeof p === 'string').slice(0, MAX_PINS);
	}
	if (o.recent && typeof o.recent === 'object') base.recent = o.recent;
	if (o.artists && typeof o.artists === 'object') base.artists = o.artists;
	if (Array.isArray(o.dismissedSeeds)) {
		base.dismissedSeeds = o.dismissedSeeds.filter((id) => typeof id === 'string');
	}
	return base;
}

// --- Shortcuts grid -------------------------------------------------------------------------------

/** Append. Returns false when it was already on the grid (its recency is refreshed instead). */
export function addPick(p: Personal, item: BrowseItem, now = Date.now()): boolean {
	const existing = p.picks.find((x) => x.id === item.id);
	if (existing) {
		existing.lastUsedAt = now;
		return false;
	}
	p.picks.push({ ...item, lastUsedAt: now });
	evictStalest(p);
	return true;
}

/**
 * Where a drag lands: put `item` immediately before `beforeId`, or at the end when it's null.
 * Handles both jobs the grid needs — moving a tile already on it, and taking one dropped in from a
 * shelf. Addressing the drop by neighbour rather than by index keeps a rightward move off by one.
 */
export function placePick(
	p: Personal,
	item: BrowseItem,
	beforeId: string | null,
	now = Date.now()
): void {
	if (beforeId === item.id) return; // dropped on itself
	const existing = p.picks.find((x) => x.id === item.id);
	const rest = p.picks.filter((x) => x.id !== item.id);
	const tile: Pick = existing ?? { ...item, lastUsedAt: now };
	const at = beforeId ? rest.findIndex((x) => x.id === beforeId) : -1;
	if (at < 0) rest.push(tile);
	else rest.splice(at, 0, tile);
	p.picks = rest;
	evictStalest(p);
}

/** Over capacity: drop the tile the user has gone longest without playing or opening. */
function evictStalest(p: Personal): void {
	while (p.picks.length > MAX_PICKS) {
		const stalest = p.picks.reduce((a, b) => (b.lastUsedAt < a.lastUsedAt ? b : a));
		p.picks = p.picks.filter((x) => x !== stalest);
	}
}

/**
 * A tile the app suggests rather than one the user added. Refused if it's already on the grid, if
 * the user has ever removed it, or if the grid is full (a suggestion never evicts something the
 * user chose). Returns whether the grid changed.
 */
export function seedPick(p: Personal, item: BrowseItem, now = Date.now()): boolean {
	if (p.picks.length >= MAX_PICKS) return false;
	if (p.dismissedSeeds.includes(item.id)) return false;
	if (p.picks.some((x) => x.id === item.id)) return false;
	p.picks.push({ ...item, lastUsedAt: now });
	return true;
}

export function removePick(p: Personal, id: string): void {
	p.picks = p.picks.filter((x) => x.id !== id);
	// Every removal is remembered, not just seeded ones: the list only ever gates `seedPick`, and
	// working out which tiles were suggested would mean storing provenance we otherwise don't need.
	// ponytail: grows one string per distinct tile ever removed, so it's bounded by how many times
	// a human clicks the ✕. Cap it if that ever stops being true.
	if (!p.dismissedSeeds.includes(id)) p.dismissedSeeds.push(id);
}

/**
 * Mark a tile as used (played or clicked). Returns whether anything changed — most calls come from
 * cards that aren't on the grid, and the caller uses this to skip a pointless write.
 */
export function touchPick(p: Personal, id: string, now = Date.now()): boolean {
	const hit = p.picks.find((x) => x.id === id);
	if (!hit) return false;
	hit.lastUsedAt = now;
	return true;
}

// --- Sidebar pins + ordering -------------------------------------------------------------------

export function togglePin(p: Personal, id: string): 'pinned' | 'unpinned' | 'full' {
	if (p.pins.includes(id)) {
		p.pins = p.pins.filter((x) => x !== id);
		return 'unpinned';
	}
	if (p.pins.length >= MAX_PINS) return 'full';
	p.pins.push(id);
	return 'pinned';
}

/**
 * Pinned first in pin order, then everything else by last played (ties and never-played items keep
 * the backend's order). Pinned ids are resolved through the live list and excluded from the tail,
 * so a playlist can never appear twice and a pin left over from a deleted playlist just vanishes.
 */
export function orderLibrary(items: BrowseItem[], p: Personal): BrowseItem[] {
	const byId = new Map(items.map((i) => [i.id, i]));
	const pinned = p.pins.map((id) => byId.get(id)).filter((i): i is BrowseItem => !!i);
	const pinnedIds = new Set(pinned.map((i) => i.id));
	const rest = items
		.map((item, index) => ({ item, index }))
		.filter(({ item }) => !pinnedIds.has(item.id))
		.sort(
			(a, b) =>
				(p.recent[b.item.id]?.at ?? 0) - (p.recent[a.item.id]?.at ?? 0) || a.index - b.index
		)
		.map(({ item }) => item);
	return [...pinned, ...rest];
}

// --- Recency + artist counts -------------------------------------------------------------------

/** Record that a playlist/album/artist was played from. */
export function noteRecent(p: Personal, item: BrowseItem, now = Date.now()): void {
	p.recent[item.id] = { ...item, at: now };
	const ids = Object.keys(p.recent);
	if (ids.length > MAX_RECENT) {
		// ponytail: newest-N window, not a history log. A real plays table is the upgrade if this
		// ever needs depth (counts, date ranges, a stats view).
		for (const id of ids.sort((a, b) => p.recent[b].at - p.recent[a].at).slice(MAX_RECENT)) {
			delete p.recent[id];
		}
	}
}

/** The most recently played-from playlists/albums/artists, newest first. */
export function recentItems(p: Personal, n = 12): RecentEntry[] {
	return Object.values(p.recent)
		.sort((a, b) => b.at - a.at)
		.slice(0, n);
}

/** The lead artist out of a joined credit string — a usable search seed, unlike the whole list. */
export function firstArtist(artists: string): string {
	return artists.split(/[,&•]|\sfeat\.?\s|\sft\.?\s/i)[0].trim();
}

export function noteArtist(p: Personal, key: string, name: string): void {
	const cur = p.artists[key];
	if (cur) {
		cur.count++;
		if (name) cur.name = name;
	} else {
		p.artists[key] = { name, count: 1 };
	}
	const keys = Object.keys(p.artists);
	if (keys.length > MAX_ARTISTS) {
		for (const k of keys
			.sort((a, b) => p.artists[b].count - p.artists[a].count)
			.slice(MAX_ARTISTS)) {
			delete p.artists[k];
		}
	}
}

/** The user's most-played artist names — the seed for the community shelf. */
export function topArtists(p: Personal, n = 3): string[] {
	return Object.values(p.artists)
		.filter((a) => a.name)
		.sort((a, b) => b.count - a.count)
		.slice(0, n)
		.map((a) => a.name);
}

/** Round-robin several result lists into one, deduped by id. */
export function interleave<T extends { id: string }>(lists: T[][], cap: number): T[] {
	const out: T[] = [];
	const seen = new Set<string>();
	const longest = lists.reduce((m, l) => Math.max(m, l.length), 0);
	for (let i = 0; i < longest && out.length < cap; i++) {
		for (const list of lists) {
			if (out.length >= cap) break;
			const item = list[i];
			if (item && !seen.has(item.id)) {
				seen.add(item.id);
				out.push(item);
			}
		}
	}
	return out;
}
