<script lang="ts">
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Search01Icon,
		UserMultiple02Icon,
		PlayIcon,
		PauseIcon,
		ShuffleIcon
	} from '@hugeicons/core-free-icons';
	import { Input } from '$lib/components/ui/input';
	import * as api from '$lib/api';
	import type { BrowseItem } from '$lib/api';
	import { auth, playback, ui, playFrom, toast } from '$lib/player.svelte';
	import { lt } from '$lib/lt.svelte';
	import { thumb } from '$lib/thumb';

	// Fixed at mount — a greeting that flips mid-session is uncanny.
	const hour = new Date().getHours();
	const daypart =
		hour < 5 ? 'Good night' : hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening';

	let searchQuery = $state('');

	function goSearch() {
		if (!searchQuery.trim()) return;
		goto(`/search?${new URLSearchParams({ q: searchQuery }).toString()}`);
	}

	// Google's CDN doesn't serve every rewritten size, so a 404'd backdrop must degrade to nothing
	// rendered, never a broken-image glyph. Re-arm whenever the track changes, mirroring MediaCard.
	let artFailed = $state(false);
	$effect(() => {
		playback.now?.thumbnail; // re-arm when the track changes
		artFailed = false;
	});

	let shuffling = $state(false);

	// Liked Music is the `VLLM` auto-playlist. Real order + `shuffle: true` — the backend owns
	// shuffling, so the player-bar toggle can still restore the true order.
	async function shuffleLiked() {
		if (shuffling) return;
		shuffling = true;
		try {
			const pl = await api.getPlaylist('VLLM');
			if (!pl.items.length) {
				toast('Nothing in Liked Music yet');
				return;
			}
			const source: BrowseItem = {
				kind: 'playlist',
				id: 'VLLM',
				title: pl.title ?? 'Liked Music',
				thumbnail: pl.thumbnail
			};
			await playFrom(source, pl.items, null, 'VLLM', true);
		} catch (e) {
			toast('Could not start Liked Music');
		} finally {
			shuffling = false;
		}
	}
</script>

<div class="relative overflow-hidden border-b">
	{#if playback.now?.thumbnail && !artFailed}
		<img
			src={thumb(playback.now.thumbnail, 1200)}
			alt=""
			class="pointer-events-none absolute inset-0 h-full w-full scale-110 object-cover opacity-60 blur-2xl"
			onerror={() => (artFailed = true)}
		/>
	{/if}
	<div
		class="absolute inset-0 bg-gradient-to-t from-background via-background/70 to-background/40"
	></div>
	<div
		class="absolute inset-0 bg-gradient-to-r from-background/80 via-background/30 to-transparent"
	></div>
	<div class="relative p-6 pt-8">
		<div class="flex items-start justify-between gap-4">
			<div class="flex min-w-0 items-center gap-3">
				{#if auth.account?.signedIn && auth.account.thumbnail}
					<!-- max-width:none defeats Tailwind Preflight's `img{max-width:100%}`, which in a tight box
					     clamps width to the content-box while height stays fixed → a vertical oval. Inline so
					     it's immune to Preflight and to stale dev CSS. -->
					<img
						src={thumb(auth.account.thumbnail, 128)}
						alt=""
						style="width:2.75rem;height:2.75rem;max-width:none"
						class="shrink-0 rounded-full object-cover ring-2 ring-border"
					/>
				{/if}
				<h1 class="truncate font-heading text-4xl font-bold tracking-tight drop-shadow">
					{daypart}{auth.account?.name ? `, ${auth.account.name.split(' ')[0]}` : ''}
				</h1>
			</div>
			<div class="flex shrink-0 items-center gap-2">
				<button
					onclick={() => (ui.ltOpen = true)}
					title="Listen Together"
					aria-label="Listen Together"
					class="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-full border transition-colors {lt.role !==
					'none'
						? 'border-primary text-primary hover:bg-primary/10'
						: 'border-border text-muted-foreground hover:bg-muted hover:text-foreground'}"
				>
					<HugeiconsIcon icon={UserMultiple02Icon} class="h-5 w-5" />
					{#if lt.role !== 'none'}
						<span
							class="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full bg-primary ring-2 ring-background"
						></span>
					{/if}
				</button>
				<form class="relative w-full max-w-xs" onsubmit={(e) => { e.preventDefault(); goSearch(); }}>
					<HugeiconsIcon
						icon={Search01Icon}
						class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
					/>
					<Input bind:value={searchQuery} placeholder="Search" class="rounded-full pl-9" />
				</form>
			</div>
		</div>
		{#if playback.now}
			<p class="mt-3 truncate text-sm text-muted-foreground">
				{playback.paused ? 'Paused' : 'Now playing'} · {playback.now.title} — {playback.now.artists}
			</p>
		{/if}
		{#if playback.now || auth.account?.signedIn}
			<div class="mt-5 flex items-center gap-3">
				{#if playback.now}
					<button
						onclick={() => api.togglePause()}
						class="flex cursor-pointer items-center gap-2 rounded-full bg-primary px-6 py-2.5 text-sm font-semibold text-primary-foreground transition hover:opacity-90 disabled:opacity-50"
					>
						<!-- HugeiconsIcon only re-renders `altIcon`/`showAlt`, not `icon` (frozen at mount) —
						     so toggle via showAlt, not a ternary on `icon`. -->
						<HugeiconsIcon
							icon={PauseIcon}
							altIcon={PlayIcon}
							showAlt={playback.paused}
							class="h-4 w-4"
						/>
						{playback.paused ? 'Resume' : 'Pause'}
					</button>
				{/if}
				{#if auth.account?.signedIn}
					<button
						onclick={shuffleLiked}
						disabled={shuffling}
						class={playback.now
							? 'flex cursor-pointer items-center gap-2 rounded-full border px-5 py-2.5 text-sm font-semibold transition hover:bg-accent/10 disabled:opacity-50'
							: 'flex cursor-pointer items-center gap-2 rounded-full bg-primary px-6 py-2.5 text-sm font-semibold text-primary-foreground transition hover:opacity-90 disabled:opacity-50'}
					>
						<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4" /> Shuffle Liked Music
					</button>
				{/if}
			</div>
		{/if}
	</div>
</div>
