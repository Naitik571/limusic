<script lang="ts">
	// Poolside Home — hero banner, mood chips, recent rail, recommendation feed.
	import { onMount } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { PlayIcon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { BrowseItem, SongItem, HomeChip } from '$lib/api';
	import { playback, playFrom, personal, toast } from '$lib/player.svelte';

	let {
		onOpenAlbum
	}: {
		onOpenAlbum: (item: BrowseItem) => void;
	} = $props();

	let chips = $state<HomeChip[]>([]);
	let sections = $state<{ title: string; items: BrowseItem[] }[]>([]);
	let recent = $state<BrowseItem[]>([]);
	let loading = $state(true);
	let feedToken = $state<string | null>(null);
	let activeChip = $state<string | null>(null);
	let sentinel = $state<HTMLDivElement | undefined>();

	onMount(async () => {
		try {
			const home = await api.getHome();
			chips = home.chips ?? [];
			sections = home.sections ?? [];
			feedToken = home.continuation ?? null;
		} catch { /* signed out or error */ }

		// recent from personal store
		const recents = Object.values(personal.recent)
			.sort((a, b) => b.at - a.at)
			.slice(0, 8)
			.map((r) => ({ kind: 'playlist' as const, id: r.id, title: r.title, thumbnail: r.thumbnail }));
		recent = recents;
		loading = false;
	});

	async function pickChip(chip: HomeChip) {
		activeChip = chip.params ?? null;
		loading = true;
		try {
			const home = await api.getHome(chip.params);
			sections = home.sections ?? [];
			feedToken = home.continuation ?? null;
		} catch { /* ignore */ }
		loading = false;
	}
	async function loadMore() {
		if (!feedToken) return;
		try {
			const more = await api.getHomeMore(feedToken);
			sections = [...sections, ...(more.sections ?? [])];
			feedToken = more.continuation ?? null;
		} catch { /* ignore */ }
	}

	function playShelfItem(item: BrowseItem) {
		if (item.kind === 'song') {
			playFrom(item, [{ video_id: item.id, title: item.title, artists: item.subtitle ?? '', thumbnail: item.thumbnail }], 0);
		} else {
			onOpenAlbum(item);
		}
	}

	// intersection observer for infinite scroll
	let observer: IntersectionObserver | undefined;
	$effect(() => {
		if (sentinel) {
			observer?.disconnect();
			observer = new IntersectionObserver((entries) => {
				if (entries[0]?.isIntersecting) loadMore();
			}, { rootMargin: '400px' });
			observer.observe(sentinel);
		}
		return () => observer?.disconnect();
	});
</script>

<div class="ps-home">
	<!-- hero -->
	<div class="ps-hero" style="background-image: {playback.now?.thumbnail ? `url(${playback.now.thumbnail})` : 'none'}">
		<div class="ps-hero-bg"></div>
		<div class="ps-hero-content ps-anim-fade-up">
			<span class="ps-hero-label">GOOD {new Date().getHours() < 12 ? 'MORNING' : new Date().getHours() < 18 ? 'AFTERNOON' : 'EVENING'}</span>
			<h1 class="ps-hero-title">DIVE IN.</h1>
			<p class="ps-hero-sub">Your music, floating in the pool.</p>
		</div>
	</div>

	<!-- mood chips -->
	{#if chips.length}
		<div class="ps-chips ps-anim-fade-up" style="animation-delay:.1s">
			<button class="ps-chip {activeChip === null ? 'on' : ''}" onclick={() => { activeChip = null; loading = true; api.getHome().then(h => { sections = h.sections ?? []; feedToken = h.continuation ?? null; loading = false; }).catch(() => { loading = false; }); }}>All</button>
			{#each chips as chip (chip.title)}
				<button class="ps-chip {activeChip === chip.params ? 'on' : ''}" onclick={() => pickChip(chip)}>{chip.title}</button>
			{/each}
		</div>
	{/if}

	<!-- recent rail -->
	{#if recent.length}
		<section class="ps-section ps-anim-fade-up" style="animation-delay:.15s">
			<h3 class="ps-section-title">JUMP BACK IN</h3>
			<div class="ps-rail">
				{#each recent as item (item.id)}
					<button class="ps-rail-card" onclick={() => onOpenAlbum(item)} title={item.title}>
						{#if item.thumbnail}
							<img src={item.thumbnail} alt={item.title} />
						{:else}
							<div class="ps-rail-placeholder"></div>
						{/if}
						<span class="ps-rail-label">{item.title}</span>
					</button>
				{/each}
			</div>
		</section>
	{/if}

	<!-- sections / feed -->
	{#each sections as section, si (section.title)}
		<section class="ps-section ps-anim-fade-up" style="animation-delay:{0.2 + si * 0.05}s">
			<h3 class="ps-section-title">{section.title.toUpperCase()}</h3>
			<div class="ps-rail">
				{#each section.items as item (item.id)}
					<button class="ps-rail-card" onclick={() => playShelfItem(item)} title={item.title}>
						{#if item.thumbnail}
							<img src={item.thumbnail} alt={item.title} />
						{:else}
							<div class="ps-rail-placeholder"></div>
						{/if}
						<span class="ps-rail-label">{item.title}</span>
						{#if item.subtitle}
							<span class="ps-rail-sub">{item.subtitle}</span>
						{/if}
					</button>
				{/each}
			</div>
		</section>
	{/each}

	{#if loading}
		<div class="ps-loading-dots">
			<span></span><span></span><span></span>
		</div>
	{/if}

	<!-- infinite scroll sentinel -->
	<div bind:this={sentinel} style="height:1px"></div>
</div>
