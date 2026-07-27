// Self-check for the pure personalization logic in `personal.ts`. There is no test runner in `ui/`
// and this doesn't warrant adding one — node 22 runs TypeScript directly:
//
//     node --experimental-strip-types ui/src/lib/personal.check.ts
//
// Prints "ok" and exits 0, or throws on the first broken invariant. Not imported by the app, so it
// never reaches the bundle.
import type { BrowseItem } from './api';
import {
	MAX_PICKS,
	addPick,
	empty,
	firstArtist,
	hydrate,
	interleave,
	noteRecent,
	orderLibrary,
	placePick,
	recentItems,
	removePick,
	seedPick,
	togglePin,
	touchPick
} from './personal.ts';

function ok(cond: boolean, what: string): void {
	if (!cond) throw new Error(`FAIL: ${what}`);
}

const item = (id: string): BrowseItem => ({ kind: 'playlist', id, title: id });
const ids = (list: { id: string }[]) => list.map((x) => x.id);
const range = (n: number, prefix: string) => Array.from({ length: n }, (_, i) => item(`${prefix}${i}`));

// --- the grid holds only what the user adds; eviction drops the stalest tile ---------------------
{
	const p = empty();
	range(MAX_PICKS, 'm').forEach((it, i) => addPick(p, it, 1000 + i));
	ok(p.picks.length === MAX_PICKS, 'the grid fills to capacity');

	addPick(p, item('newest'), 9000);
	ok(p.picks.length === MAX_PICKS, 'an add over capacity keeps the grid at 18');
	ok(!ids(p.picks).includes('m0'), 'the least recently used tile is evicted');
	ok(ids(p.picks).includes('m1') && ids(p.picks).includes('newest'), 'the rest survive');

	// Playing a tile protects it: it is no longer the stalest.
	touchPick(p, 'm1', 9500);
	addPick(p, item('another'), 9600);
	ok(ids(p.picks).includes('m1'), 'a recently played tile is not evicted');
	ok(!ids(p.picks).includes('m2'), 'the next-stalest goes instead');
}

// --- adding an existing tile refreshes it rather than duplicating --------------------------------
{
	const p = empty();
	addPick(p, item('a'), 100);
	addPick(p, item('b'), 200);
	ok(addPick(p, item('a'), 300) === false, 'a repeat add reports "already there"');
	ok(p.picks.length === 2, 'and does not duplicate the tile');
	ok(p.picks.find((x) => x.id === 'a')!.lastUsedAt === 300, 'but does refresh its recency');
}

// --- removal is permanent: nothing refills the grid ----------------------------------------------
{
	const p = empty();
	range(4, 'm').forEach((it, i) => addPick(p, it, 1000 + i));
	removePick(p, 'm2');
	ok(p.picks.length === 3, 'removal takes the tile out');
	ok(!ids(p.picks).includes('m2'), 'and it stays out');
	removePick(p, 'm0');
	removePick(p, 'm1');
	removePick(p, 'm3');
	ok(p.picks.length === 0, 'the grid can be emptied completely');
}

// --- display order is array order: additions append, use never reshuffles ------------------------
{
	const p = empty();
	range(6, 'c').forEach((it, i) => addPick(p, it, 1000 + i));
	ok(ids(p.picks).join() === 'c0,c1,c2,c3,c4,c5', 'tiles appear in the order they were added');
	touchPick(p, 'c4', 99999);
	ok(ids(p.picks).join() === 'c0,c1,c2,c3,c4,c5', 'playing a tile does not move it');
}

// --- placePick: what a drag lands on. Ordering is the user's, so this is the load-bearing bit -----
{
	const p = empty();
	range(4, 'd').forEach((it, i) => addPick(p, it, 1000 + i)); // d0,d1,d2,d3

	// Leftward move: in front of the tile it was dropped on.
	placePick(p, item('d3'), 'd1');
	ok(ids(p.picks).join() === 'd0,d3,d1,d2', 'a tile moves in front of the drop target');

	// Rightward move: also in front of the target — no off-by-one from the tile leaving its old slot.
	placePick(p, item('d0'), 'd2');
	ok(ids(p.picks).join() === 'd3,d1,d0,d2', 'a rightward move lands in front of the target too');

	// Null target = the end of the grid.
	placePick(p, item('d3'), null);
	ok(ids(p.picks).join() === 'd1,d0,d2,d3', 'a drop past the last tile appends');

	// Dropped on itself: a no-op, not a duplicate or a jump to the end.
	placePick(p, item('d0'), 'd0');
	ok(ids(p.picks).join() === 'd1,d0,d2,d3', 'dropping a tile on itself changes nothing');
	ok(p.picks.length === 4, 'and never duplicates it');

	// A tile dragged in from a shelf inserts at the drop point.
	placePick(p, item('fresh'), 'd2', 5000);
	ok(ids(p.picks).join() === 'd1,d0,fresh,d2,d3', 'a new tile inserts where it was dropped');
	ok(p.picks.find((x) => x.id === 'fresh')!.lastUsedAt === 5000, 'and starts out fresh');
}

// --- placePick respects capacity, evicting the stalest rather than the new arrival ----------------
{
	const p = empty();
	range(MAX_PICKS, 'f').forEach((it, i) => addPick(p, it, 1000 + i));
	placePick(p, item('dropped'), 'f5', 9000);
	ok(p.picks.length === MAX_PICKS, 'an insert over capacity keeps the grid at 18');
	ok(ids(p.picks).includes('dropped'), 'the tile just dropped in survives');
	ok(!ids(p.picks).includes('f0'), 'the stalest tile is the one evicted');
	ok(ids(p.picks)[4] === 'dropped', 'and it sits where it was dropped');

	// Rearranging a full grid must not evict anything.
	placePick(p, item('f9'), null);
	ok(p.picks.length === MAX_PICKS, 'moving a tile on a full grid evicts nothing');
	ok(ids(p.picks).at(-1) === 'f9', 'it just moves to the end');
}

// --- pins: capped at 3, order preserved ---------------------------------------------------------
{
	const p = empty();
	ok(togglePin(p, 'a') === 'pinned' && togglePin(p, 'b') === 'pinned', 'first two pins take');
	ok(togglePin(p, 'c') === 'pinned', 'third pin takes');
	ok(togglePin(p, 'd') === 'full', 'a fourth pin is refused');
	ok(p.pins.join() === 'a,b,c', 'pin order is insertion order');
	ok(togglePin(p, 'b') === 'unpinned', 'toggling an existing pin unpins it');
	ok(p.pins.join() === 'a,c', 'unpinning preserves the order of the rest');
	ok(togglePin(p, 'd') === 'pinned', 'a slot freed by unpinning is usable');
}

// --- library ordering: pins first, then last played, no duplicates ------------------------------
{
	const p = empty();
	const items = [item('a'), item('b'), item('c'), item('d')];
	togglePin(p, 'c');
	noteRecent(p, item('b'), 100);
	noteRecent(p, item('d'), 50);
	const ordered = orderLibrary(items, p);
	ok(ids(ordered).join() === 'c,b,d,a', 'pinned first, then most recently played, then untouched');
	ok(new Set(ids(ordered)).size === ordered.length, 'no playlist appears twice');
	ok(ordered.length === items.length, 'nothing is dropped');

	togglePin(p, 'zzz'); // a pin whose playlist is gone
	ok(orderLibrary(items, p).length === items.length, 'a stale pin does not duplicate or crash');

	// A pinned playlist that is also the most recently played must still appear exactly once.
	noteRecent(p, item('c'), 999);
	const dupCheck = orderLibrary(items, p);
	ok(ids(dupCheck).filter((id) => id === 'c').length === 1, 'pinned + recent is not duplicated');
}

// --- recentItems: newest first, capped, empty when nothing played --------------------------------
{
	const p = empty();
	ok(recentItems(p).length === 0, 'no recent activity yields an empty list');

	noteRecent(p, item('a'), 100);
	noteRecent(p, item('b'), 300);
	noteRecent(p, item('c'), 200);
	ok(ids(recentItems(p)).join() === 'b,c,a', 'newest played-from comes first');
	ok(ids(recentItems(p, 2)).join() === 'b,c', 'n caps the result');
}

// --- interleave dedupes across lists ------------------------------------------------------------
{
	const merged = interleave([[item('x'), item('y')], [item('x'), item('z')]], 10);
	ok(ids(merged).join() === 'x,y,z', 'round-robins and drops repeats');
	ok(interleave([range(9, 'a'), range(9, 'b')], 4).length === 4, 'the cap holds');
}

// --- artist credit parsing ----------------------------------------------------------------------
{
	ok(firstArtist('Daft Punk') === 'Daft Punk', 'a lone artist is unchanged');
	ok(firstArtist('Daft Punk, Pharrell Williams') === 'Daft Punk', 'a comma list takes the lead');
	ok(firstArtist('The Weeknd & Ariana Grande') === 'The Weeknd', 'an ampersand pair takes the lead');
	ok(firstArtist('Drake feat. Rihanna') === 'Drake', 'a feature credit is stripped');
}

// --- hydrate survives junk ----------------------------------------------------------------------
{
	ok(hydrate(null).picks.length === 0, 'null hydrates to empty');
	ok(hydrate('nonsense').pins.length === 0, 'a bad blob hydrates to empty');
	ok(hydrate({ pins: ['a', 'b', 'c', 'd', 'e'] }).pins.length === 3, 'an over-long pin list is cut');
	ok(hydrate({ picks: [{ id: 'a' }, {}] }).picks.length === 1, 'malformed tiles are dropped');

	// Migration off the auto-seeding build: `manual: false` tiles were never chosen by the user.
	const migrated = hydrate({
		picks: [{ id: 'kept', manual: true }, { id: 'seeded', manual: false }, { id: 'new' }]
	});
	ok(ids(migrated.picks).join() === 'kept,new', 'auto-seeded tiles from the old build are dropped');
	ok(hydrate({ dismissedSeeds: ['a', 7] }).dismissedSeeds.join() === 'a', 'junk dismissals drop');
}

// --- a seeded tile is a suggestion: never forced, and never suggested twice ----------------------
{
	const p = empty();
	ok(seedPick(p, item('onrepeat')), 'an empty grid takes the suggestion');
	ok(!seedPick(p, item('onrepeat')), 'a suggestion already on the grid is not re-added');

	// The whole point of the dismissal: removing it has to stick across every later attempt.
	removePick(p, 'onrepeat');
	ok(p.picks.length === 0, 'removing the tile takes it off the grid');
	ok(!seedPick(p, item('onrepeat')), 'a removed suggestion never comes back');
	ok(!seedPick(p, item('onrepeat'), 9999), 'not on a later visit either');

	// A dismissal only gates suggestions. The user can still put it back by hand, and removing it
	// again re-arms the dismissal rather than leaving it stuck on.
	addPick(p, item('onrepeat'));
	ok(ids(p.picks).join() === 'onrepeat', 'the user can still add it manually');
	removePick(p, 'onrepeat');
	ok(!seedPick(p, item('onrepeat')), 'and removing it again still blocks the suggestion');
}

// --- a suggestion must never push a hand-picked tile off a full grid -----------------------------
{
	const p = empty();
	range(MAX_PICKS, 'mine').forEach((it, i) => addPick(p, it, 1000 + i));
	ok(!seedPick(p, item('onrepeat')), 'a full grid refuses the suggestion');
	ok(p.picks.length === MAX_PICKS && !ids(p.picks).includes('onrepeat'), 'nothing was evicted');
}

console.log('ok');
