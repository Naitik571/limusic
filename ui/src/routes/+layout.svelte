<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import { ModeWatcher } from 'mode-watcher';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		CheckmarkCircle02Icon,
		AlertCircleIcon,
		InformationCircleIcon
	} from '@hugeicons/core-free-icons';
	import { browser } from '$app/environment';
	import { onMount } from 'svelte';
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { initTheme } from '$lib/theme.svelte';
	import { dragScroll } from '$lib/dnd';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import Titlebar from '$lib/components/Titlebar.svelte';
	import ResizeBorders from '$lib/components/ResizeBorders.svelte';
	import PlayerBar from '$lib/components/PlayerBar.svelte';
	import QueuePanel from '$lib/components/QueuePanel.svelte';
	import LyricsPanel from '$lib/components/LyricsPanel.svelte';
	import AddToPlaylist from '$lib/components/AddToPlaylist.svelte';
	import SettingsDialog from '$lib/components/SettingsDialog.svelte';
	import ListenTogether from '$lib/components/ListenTogether.svelte';
	import { Button } from '$lib/components/ui/button';
	import { auth, initApp, playback, ui } from '$lib/player.svelte';
	import { win, initWin } from '$lib/win.svelte';
	import { updateState, installUpdate, checkForUpdatesQuiet } from '$lib/updater.svelte';

	let { children } = $props();
	// Queue and lyrics toggle independently — at lg+ they dock as two side-by-side columns, and
	// expanded lyrics leaves room for the queue column (see LyricsPanel).
	let queueOpen = $state(false);
	let lyricsOpen = $state(false);

	// Apply the saved accent color before the first paint (ssr=false → nothing renders until now).
	if (browser) initTheme();

	// Wire the Tauri event bridge once for the whole app; teardown on destroy. Check for an update
	// on every app open (silent unless one exists).
	onMount(() => {
		checkForUpdatesQuiet();
		const teardownApp = initApp();
		const teardownWin = initWin();
		return () => {
			teardownApp();
			teardownWin();
		};
	});
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>
<ModeWatcher />

<!-- The window itself is transparent; this root paints the background and, when not maximized,
     rounds the corners (the compositor can't round an undecorated window for us). -->
<div
	class="flex h-screen flex-col overflow-hidden bg-background text-foreground {win.maximized
		? ''
		: 'rounded-lg'}"
>
	<ResizeBorders />
	<Titlebar />
	<!-- relative: lets QueuePanel overlay the content on narrow windows (see QueuePanel). -->
	<div class="relative flex min-h-0 flex-1">
		<Sidebar />
		<!-- dragScroll: dragging a card up to home's Shortcuts grid has to be possible from anywhere in
		     the feed, so aiming at the top edge scrolls this container while the drag is in flight. -->
		<main class="min-w-0 flex-1 overflow-y-auto" {@attach dragScroll}>
			<!-- Remount the current page on sign-in/out so it refetches with the new account. -->
			{#key auth.epoch}
				{@render children()}
			{/key}
		</main>
		<!-- Lyrics before queue: docked side by side, lyrics takes the left column, queue the right. -->
		{#if lyricsOpen}<LyricsPanel onClose={() => (lyricsOpen = false)} {queueOpen} />{/if}
		{#if queueOpen}<QueuePanel onClose={() => (queueOpen = false)} />{/if}
	</div>
	{#if playback.now}
		<!-- Slides up from its own height on first play; leaves instantly (bar removal is rare). -->
		<div in:fly={{ y: 64, duration: 250, easing: cubicOut }}>
			<PlayerBar
				onToggleQueue={() => (queueOpen = !queueOpen)}
				{queueOpen}
				onToggleLyrics={() => (lyricsOpen = !lyricsOpen)}
				{lyricsOpen}
			/>
		</div>
	{/if}
</div>

<AddToPlaylist />
<SettingsDialog />
<ListenTogether />

<!-- The three notification banners below run at z-[100]. Dialogs and menus sit at z-50 and portal to
     <body>, so a z-50 banner loses the tie on DOM order and hides behind an open modal. -->
{#if updateState.available}
	<div
		transition:fly={{ y: 16, duration: 220, easing: cubicOut }}
		class="fixed bottom-24 left-1/2 z-[100] flex -translate-x-1/2 items-center gap-3 rounded-lg border bg-card px-4 py-2 text-sm shadow-lg"
	>
		<span>Update available — v{updateState.available.version}</span>
		<Button size="sm" onclick={installUpdate} disabled={updateState.installing}>
			{updateState.installing ? 'Updating…' : 'Update now'}
		</Button>
		{#if !updateState.installing}
			<button
				class="text-muted-foreground hover:text-foreground"
				aria-label="Dismiss"
				onclick={() => (updateState.available = null)}>✕</button
			>
		{/if}
	</div>
{/if}

{#if ui.toast}
	{@const t = ui.toast}
	<div
		transition:fly={{ y: 16, duration: 220, easing: cubicOut }}
		class="fixed bottom-40 left-1/2 z-[100] flex -translate-x-1/2 items-center gap-2 rounded-lg border bg-card px-4 py-2 text-sm shadow-lg"
	>
		<!-- Three branches instead of a ternary on `icon`: HugeiconsIcon freezes `icon` at mount, so a
		     new toast replacing a visible one would keep the old glyph. -->
		{#if t.kind === 'success'}
			<HugeiconsIcon icon={CheckmarkCircle02Icon} class="h-4 w-4 shrink-0 text-primary" />
		{:else if t.kind === 'error'}
			<HugeiconsIcon icon={AlertCircleIcon} class="h-4 w-4 shrink-0 text-destructive" />
		{:else}
			<HugeiconsIcon icon={InformationCircleIcon} class="h-4 w-4 shrink-0 text-muted-foreground" />
		{/if}
		{t.msg}
	</div>
{/if}

{#if playback.error}
	<div
		transition:fly={{ y: 16, duration: 220, easing: cubicOut }}
		class="fixed bottom-24 left-1/2 z-[100] flex -translate-x-1/2 items-center gap-3 rounded-lg border border-destructive/40 bg-card px-4 py-2 text-sm text-destructive shadow-lg"
	>
		<span>{playback.error}</span>
		<button
			class="text-muted-foreground hover:text-foreground"
			aria-label="Dismiss"
			onclick={() => (playback.error = null)}>✕</button
		>
	</div>
{/if}
