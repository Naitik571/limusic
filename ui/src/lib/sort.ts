import type { SongItem } from './api';

export type SortMode =
	| 'default'
	| 'title'
	| 'title-desc'
	| 'artist'
	| 'artist-desc'
	| 'album'
	| 'album-desc'
	| 'duration'
	| 'duration-desc'
	| 'date'
	| 'date-desc'
	| 'plays'
	| 'plays-desc';

export const SORT_OPTIONS: { value: SortMode; label: string }[] = [
	{ value: 'default', label: 'Default' },
	{ value: 'title', label: 'Title (A→Z)' },
	{ value: 'title-desc', label: 'Title (Z→A)' },
	{ value: 'artist', label: 'Artist (A→Z)' },
	{ value: 'artist-desc', label: 'Artist (Z→A)' },
	{ value: 'album', label: 'Album (A→Z)' },
	{ value: 'album-desc', label: 'Album (Z→A)' },
	{ value: 'duration', label: 'Duration (shortest)' },
	{ value: 'duration-desc', label: 'Duration (longest)' },
	{ value: 'date', label: 'Date added (oldest)' },
	{ value: 'date-desc', label: 'Date added (newest)' },
	{ value: 'plays', label: 'Most played (least)' },
	{ value: 'plays-desc', label: 'Most played (most)' }
];

// Collations
const ignoreArticles = (s: string) =>
	s
		.toLowerCase()
		.replace(/^the\s+/, '')
		.replace(/^a\s+/, '')
		.replace(/^an\s+/, '');

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

// Extract a stable "added" order from set_video_id. YouTube's set video ids are of the form
// `VIDEO_ID-EPOCH_SECONDS`; a larger number means it was added later. Songs without one fall
// back to their position in the list (overwrite sort can't re-order those anyway).
const addedAt = (t: SongItem): number => {
	const s = t.set_video_id ?? '';
	const m = /-(\d+)$/.exec(s);
	return m ? Number(m[1]) : 0;
};

export function compareSongs(a: SongItem, b: SongItem, mode: SortMode): number {
	switch (mode) {
		case 'title':
		case 'title-desc':
			return collator.compare(ignoreArticles(a.title), ignoreArticles(b.title));
		case 'artist':
		case 'artist-desc':
			return collator.compare(ignoreArticles(a.artists), ignoreArticles(b.artists));
		case 'album':
		case 'album-desc':
			return collator.compare(ignoreArticles(a.album ?? ''), ignoreArticles(b.album ?? ''));
		case 'duration':
		case 'duration-desc':
			return durToSec(a.duration) - durToSec(b.duration);
		case 'date':
		case 'date-desc':
			return addedAt(a) - addedAt(b);
		case 'plays':
		case 'plays-desc': {
			const pa = (a as SongItem & { _plays?: number })._plays ?? 0;
			const pb = (b as SongItem & { _plays?: number })._plays ?? 0;
			return pa - pb;
		}
		default:
			return 0; // 'default' keeps the existing order
	}
}

/** Sort `items` in place (stable) per `mode`. `plays` maps videoId → play count for the
 *  "most played" modes; it is attached as `_plays` before sorting so the comparator can read it. */
export function sortItems(items: SongItem[], mode: SortMode, plays?: Record<string, number>): void {
	if (mode === 'default') return;
	const desc = mode.endsWith('-desc');
	const base: SortMode = (desc ? mode.slice(0, -5) : mode) as SortMode;
	if (plays && base === 'plays') {
		for (const t of items) (t as SongItem & { _plays?: number })._plays = plays[t.video_id] ?? 0;
	}
	const cmp = (a: SongItem, b: SongItem) => compareSongs(a, b, base);
	// Stable ascending sort on the base key.
	items.sort((a, b) => cmp(a, b));
	// For *-desc variants, reverse to get descending order while keeping tie stability.
	if (desc) items.reverse();
}

// "3:21" / "1:02:03" → seconds (the SongItem duration is a display string, not a number).
function durToSec(d: string | undefined): number {
	if (!d) return 0;
	let total = 0;
	for (const part of d.split(':')) {
		const n = Number(part);
		if (Number.isNaN(n)) return 0;
		total = total * 60 + n;
	}
	return total;
}
