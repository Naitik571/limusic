<!--
  RadioBrowse — radio station browser.

  Layout:
    - Top: search bar
    - Tab row: My Stations | Global | Favorites
    - Grid of station cards (3-column responsive). Each card:
        * cover image (placeholder gradient if none)
        * "ONLINE" green badge or red "LIVE" badge
        * station name + genre/location
        * star icon in the top-right corner (toggle favorite)
    - "Load More Stations" button at the bottom (paginates in more)
-->
<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Search01Icon, StarIcon, SearchFocusIcon } from '@hugeicons/core-free-icons';
	import { onMount } from 'svelte';
	import { playback, toast } from '$lib/player.svelte';

	type Tab = 'mine' | 'global' | 'favorites';
	type Station = {
		id: string;
		name: string;
		genre: string;
		location: string;
		cover?: string;
		isLive: boolean;
		listeners?: number;
		isFavorite: boolean;
		isMine: boolean;
	};

	let activeTab = $state<Tab>('global');
	let query = $state('');
	let page = $state(0);
	const PAGE_SIZE = 12;

	// Mock station data. Real radio backend is not in the Rust side, so we
	// generate a stable list with seeded colors so the UI is fully wired even
	// before the backend is connected.
	const STATION_NAMES = [
		'Lush FM', 'Deep House Tokyo', 'Indie Underground', 'Jazz After Dark',
		'Lo-Fi Cafe', 'Classical FM', 'K-Pop Hot 100', 'Vinyl Sessions',
		'Liquid Soul', 'Synthwave Drive', 'Acoustic Mornings', 'Bass Nation',
		'Rainforest Sounds', 'Late Night Jazz', 'Morning Coffee', 'Tokyo Underground',
		'Bay Area Bumps', 'Detroit Techno', 'Chicago House', 'Sunset Lounge',
		'Highway 61', 'Pulse FM', 'Nirvana Radio', 'Blue Note Live'
	];
	const GENRES = ['Lo-Fi', 'House', 'Indie', 'Jazz', 'Classical', 'K-Pop', 'Disco', 'Soul', 'Synthwave', 'Acoustic', 'Bass', 'Ambient'];
	const LOCATIONS = ['Tokyo', 'Berlin', 'London', 'New York', 'São Paulo', 'Seoul', 'Paris', 'Amsterdam', 'Stockholm', 'Mexico City', 'Lagos', 'Mumbai'];

	function makeStations(start: number, count: number): Station[] {
		const out: Station[] = [];
		for (let i = 0; i < count; i++) {
			const idx = start + i;
			const name = STATION_NAMES[idx % STATION_NAMES.length];
			const genre = GENRES[(idx * 7) % GENRES.length];
			const loc = LOCATIONS[(idx * 11) % LOCATIONS.length];
			out.push({
				id: `station-${idx}`,
				name: `${name} ${idx >= STATION_NAMES.length ? Math.floor(idx / STATION_NAMES.length) + 1 : ''}`.trim(),
				genre,
				location: loc,
				isLive: idx % 3 !== 0,
				listeners: 200 + ((idx * 173) % 4000),
				isFavorite: (idx * 13) % 7 === 0,
				isMine: idx < 3
			});
		}
		return out;
	}

	let stations = $state<Station[]>(makeStations(0, PAGE_SIZE));
	let favorites = $state<Set<string>>(new Set());

	const filtered = $derived.by(() => {
		const q = query.trim().toLowerCase();
		return stations.filter((s) => {
			if (activeTab === 'mine' && !s.isMine) return false;
			if (activeTab === 'favorites' && !favorites.has(s.id)) return false;
			if (q && !s.name.toLowerCase().includes(q) && !s.genre.toLowerCase().includes(q)) return false;
			return true;
		});
	});

	function loadMore() {
		page += 1;
		const start = stations.length;
		stations = [...stations, ...makeStations(start, PAGE_SIZE)];
		toast.info(`Loaded ${PAGE_SIZE} more stations`);
	}
	function toggleFavorite(s: Station, e: MouseEvent) {
		e.stopPropagation();
		const next = new Set(favorites);
		if (next.has(s.id)) {
			next.delete(s.id);
			toast.info(`Removed ${s.name} from favorites`);
		} else {
			next.add(s.id);
			toast.success(`Added ${s.name} to favorites`);
		}
		favorites = next;
	}
	function playStation(s: Station) {
		toast.info(`Now playing: ${s.name}`);
	}
	// generate a stable background gradient for stations without covers
	function coverGradient(seed: number): string {
		const h = (seed * 47) % 360;
		const h2 = (seed * 89 + 60) % 360;
		return `linear-gradient(135deg, hsl(${h} 60% 45%), hsl(${h2} 70% 35%))`;
	}
</script>

<div class="ps-radio">
	<header class="ps-radio-top">
		<div class="ps-radio-search">
			<HugeiconsIcon icon={Search01Icon} />
			<input
				bind:value={query}
				type="search"
				placeholder="SEARCH RADIO..."
				aria-label="Search radio stations"
			/>
		</div>
	</header>

	<div class="ps-radio-tabs ps-tab-track" role="tablist">
		<div
			class="ps-tab-pill"
			style="left: {4 + (activeTab === 'mine' ? 0 : activeTab === 'global' ? 110 : 220)}px; width: 100px;"
		></div>
		<button class:on={activeTab === 'mine'} onclick={() => (activeTab = 'mine')} role="tab">
			My Stations
		</button>
		<button class:on={activeTab === 'global'} onclick={() => (activeTab = 'global')} role="tab">
			Global
		</button>
		<button class:on={activeTab === 'favorites'} onclick={() => (activeTab = 'favorites')} role="tab">
			Favorites
		</button>
	</div>

	<div class="ps-radio-grid">
		{#each filtered as s (s.id)}
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
			<div
				class="ps-radio-card"
				role="button"
				tabindex="0"
				onclick={() => playStation(s)}
				onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), playStation(s))}
			>
				<div class="ps-radio-cover" style={s.cover ? `background-image: url('${s.cover}')` : `background: ${coverGradient(parseInt(s.id.split('-')[1] || '0'))}`}>
					{#if s.isLive}
						<span class="ps-radio-badge ps-radio-badge--live">
							<span class="ps-radio-badge-dot"></span>LIVE
						</span>
					{:else}
						<span class="ps-radio-badge ps-radio-badge--online">ONLINE</span>
					{/if}
					<button
						class="ps-radio-star {favorites.has(s.id) ? 'is-fav' : ''}"
						onclick={(e) => toggleFavorite(s, e)}
						aria-label={favorites.has(s.id) ? 'Remove from favorites' : 'Add to favorites'}
					>
						<HugeiconsIcon icon={StarIcon} />
					</button>
				</div>
				<div class="ps-radio-info">
					<span class="ps-radio-name">{s.name}</span>
					<span class="ps-radio-meta">{s.genre} · {s.location}</span>
					{#if s.listeners}
						<span class="ps-radio-listeners">{s.listeners.toLocaleString()} listening</span>
						{/if}
						</div>
						</div>
						{:else}
						<div class="ps-radio-empty">
						<HugeiconsIcon icon={SearchFocusIcon} class="w-10 h-10 opacity-40" />
						<span>No stations match.</span>
						</div>
						{/each}
	</div>

	<div class="ps-radio-more">
		<button class="ps-ghost" onclick={loadMore}>
			Load More Stations
		</button>
	</div>
</div>

<style>
	.ps-radio {
		padding: 78px 32px 110px;
		height: 100%;
		overflow-y: auto;
		scrollbar-width: thin;
	}
	.ps-radio-top { display: flex; justify-content: center; margin-bottom: 18px; }
	.ps-radio-search {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 360px;
		max-width: 520px;
		flex: 1;
		background: rgba(255, 255, 255, 0.12);
		backdrop-filter: blur(14px);
		border: 1px solid rgba(255, 255, 255, 0.25);
		border-radius: 999px;
		padding: 10px 18px;
	}
	.ps-radio-search svg { width: 18px; height: 18px; opacity: 0.7; flex: none; }
	.ps-radio-search input {
		all: unset;
		flex: 1;
		font-size: 11px;
		letter-spacing: 0.18em;
		text-transform: uppercase;
	}
	.ps-radio-search input::placeholder { color: rgba(255, 255, 255, 0.55); }
	.ps-radio-tabs { display: flex; justify-content: center; gap: 0; margin-bottom: 24px; }
	.ps-radio-tabs button {
		all: unset;
		cursor: pointer;
		min-width: 100px;
		text-align: center;
		padding: 10px 16px;
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.22em;
		text-transform: uppercase;
		color: rgba(255, 255, 255, 0.65);
		border-radius: 999px;
		position: relative;
		z-index: 1;
		transition: color 0.2s;
	}
	.ps-radio-tabs button.on { color: #0e6a7a; }
	.ps-radio-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: 20px;
		max-width: 1200px;
		margin: 0 auto;
	}
	.ps-radio-card {
		all: unset;
		cursor: pointer;
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 12px;
		border-radius: 16px;
		background: rgba(255, 255, 255, 0.08);
		backdrop-filter: blur(12px);
		border: 1px solid rgba(255, 255, 255, 0.18);
		transition: transform 0.25s cubic-bezier(0.22, 1, 0.36, 1), background 0.2s;
	}
	.ps-radio-card:hover {
		transform: translateY(-4px);
		background: rgba(255, 255, 255, 0.15);
	}
	.ps-radio-cover {
		position: relative;
		aspect-ratio: 1;
		border-radius: 12px;
		background-color: #0a0a0a;
		background-size: cover;
		background-position: center;
		border: 1.5px solid rgba(255, 255, 255, 0.3);
		overflow: hidden;
	}
	.ps-radio-badge {
		position: absolute;
		top: 8px;
		left: 8px;
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 4px 8px;
		font-size: 8px;
		font-weight: 700;
		letter-spacing: 0.18em;
		border-radius: 4px;
	}
	.ps-radio-badge--live {
		background: #e02020;
		color: #fff;
	}
	.ps-radio-badge--online {
		background: #4ade80;
		color: #062c1a;
	}
	.ps-radio-badge-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: #fff;
		animation: ps-radio-live-pulse 1.4s ease-in-out infinite;
	}
	@keyframes ps-radio-live-pulse {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.4; transform: scale(0.7); }
	}
	.ps-radio-star {
		all: unset;
		cursor: pointer;
		position: absolute;
		top: 8px;
		right: 8px;
		width: 30px;
		height: 30px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		background: rgba(0, 0, 0, 0.4);
		color: #fff;
		opacity: 0.7;
		transition: opacity 0.2s, color 0.2s, transform 0.2s;
	}
	.ps-radio-star:hover { opacity: 1; transform: scale(1.1); }
	.ps-radio-star.is-fav {
		color: #ffd54a;
		opacity: 1;
	}
	.ps-radio-star svg { width: 14px; height: 14px; }
	.ps-radio-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.ps-radio-name {
		font-size: 12px;
		font-weight: 700;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-radio-meta {
		font-size: 9px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		opacity: 0.65;
	}
	.ps-radio-listeners {
		font-size: 9px;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		opacity: 0.55;
	}
	.ps-radio-empty {
		grid-column: 1 / -1;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
		padding: 60px 0;
		opacity: 0.6;
		font-size: 11px;
		letter-spacing: 0.18em;
		text-transform: uppercase;
	}
	.ps-radio-more {
		display: flex;
		justify-content: center;
		margin-top: 32px;
	}
</style>
