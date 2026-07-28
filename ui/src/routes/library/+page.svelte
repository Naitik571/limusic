<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Add01Icon,
		DriveIcon,
		MusicNoteSquare02Icon,
		Playlist02Icon,
		SquareStackIcon,
		UserSharingIcon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Tabs from '$lib/components/ui/tabs';
	import LocalMusic from '$lib/components/LocalMusic.svelte';
	import MediaCard from '$lib/components/MediaCard.svelte';
	import MediaCardSkeleton from '$lib/components/MediaCardSkeleton.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import * as api from '$lib/api';
	import type { BrowseItem } from '$lib/api';
	import { auth, toast, library, loadLibrary, createLibraryPlaylist } from '$lib/player.svelte';

	let dialogOpen = $state(false);
	let newTitle = $state('');
	let busy = $state(false);
	// `?tab=local` so anything that sends you back here (an album whose files were deleted) lands
	// on the tab you came from instead of a sign-in prompt.
	let tab = $state(page.url.searchParams.get('tab') ?? 'all');

	// Playlists live in the shared `library` store (the sidebar renders them too). Albums and
	// artists have this page as their only consumer, so they stay local and refresh on each visit.
	let albums = $state<BrowseItem[]>([]);
	let artists = $state<BrowseItem[]>([]);
	let extraLoading = $state(false);
	let extraError = $state<string | null>(null);

	const all = $derived([...library.items, ...albums, ...artists]);
	const loading = $derived((library.loading || extraLoading) && !all.length);
	const error = $derived(library.error ?? extraError);

	onMount(() => {
		if (auth.account?.signedIn) load();
	});

	function load() {
		loadLibrary(true);
		loadExtras();
	}

	async function loadExtras() {
		extraLoading = true;
		extraError = null;
		try {
			[albums, artists] = await Promise.all([api.getLibraryAlbums(), api.getLibraryArtists()]);
		} catch (e) {
			extraError = String(e);
		} finally {
			extraLoading = false;
		}
	}

	async function createNew() {
		const title = newTitle.trim();
		if (!title || busy) return;
		busy = true;
		try {
			await createLibraryPlaylist(title);
			toast(`Created "${title}"`);
			newTitle = '';
			dialogOpen = false;
		} catch (e) {
			toast(String(e));
		} finally {
			busy = false;
		}
	}
</script>

{#snippet grid(items: BrowseItem[], empty: string)}
	{#if items.length}
		<div class="content-in grid grid-cols-[repeat(auto-fill,10rem)] gap-4">
			{#each items as item (item.kind + item.id)}
				<MediaCard {item} />
			{/each}
		</div>
	{:else}
		<p class="text-sm text-muted-foreground">{empty}</p>
	{/if}
{/snippet}

<div class="p-6">
	<div class="mb-6 flex items-center justify-between">
		<h1 class="font-heading text-2xl font-bold">Library</h1>
		{#if auth.account?.signedIn}
			<Button variant="outline" size="sm" class="gap-2" onclick={() => (dialogOpen = true)}>
				<HugeiconsIcon icon={Add01Icon} class="h-4 w-4" /> New playlist
			</Button>
		{/if}
	</div>

	<Dialog.Root bind:open={dialogOpen}>
		<Dialog.Content class="sm:max-w-md">
			<Dialog.Header>
				<Dialog.Title>New playlist</Dialog.Title>
				<Dialog.Description>Give your playlist a name to get started.</Dialog.Description>
			</Dialog.Header>
			<form
				class="flex flex-col gap-4"
				onsubmit={(e) => {
					e.preventDefault();
					createNew();
				}}
			>
				<Input bind:value={newTitle} placeholder="Playlist name" autofocus />
				<Dialog.Footer>
					<Button type="button" variant="outline" onclick={() => (dialogOpen = false)}>
						Cancel
					</Button>
					<Button type="submit" disabled={busy || !newTitle.trim()}>
						{busy ? 'Creating…' : 'Create'}
					</Button>
				</Dialog.Footer>
			</form>
		</Dialog.Content>
	</Dialog.Root>

	<!-- The tabs always render: Local music needs neither an account nor a connection. -->
	<Tabs.Root bind:value={tab}>
		<Tabs.List class="mb-4">
			<Tabs.Trigger value="all">
				<HugeiconsIcon icon={SquareStackIcon} class="h-4 w-4" /> All
			</Tabs.Trigger>
			<Tabs.Trigger value="playlists">
				<HugeiconsIcon icon={Playlist02Icon} class="h-4 w-4" /> Playlists
			</Tabs.Trigger>
			<Tabs.Trigger value="albums">
				<HugeiconsIcon icon={MusicNoteSquare02Icon} class="h-4 w-4" /> Albums
			</Tabs.Trigger>
			<Tabs.Trigger value="artists">
				<HugeiconsIcon icon={UserSharingIcon} class="h-4 w-4" /> Artists
			</Tabs.Trigger>
			<Tabs.Trigger value="local">
				<HugeiconsIcon icon={DriveIcon} class="h-4 w-4" /> Local
			</Tabs.Trigger>
		</Tabs.List>
		{#if tab === 'local'}
			<LocalMusic />
		{:else if !auth.account?.signedIn}
			<p class="text-sm text-muted-foreground">
				Sign in to see your playlists and liked songs, or open the Local tab for music on this
				device.
			</p>
		{:else if loading}
			<div class="grid grid-cols-[repeat(auto-fill,10rem)] gap-4">
				{#each Array(12) as _, i (i)}
					<MediaCardSkeleton />
				{/each}
			</div>
		{:else if error}
			<ErrorState message={error} onRetry={load} />
		{:else}
			<Tabs.Content value="all">{@render grid(all, 'Your library is empty.')}</Tabs.Content>
			<Tabs.Content value="playlists">
				{@render grid(library.items, 'No playlists yet.')}
			</Tabs.Content>
			<Tabs.Content value="albums">
				{@render grid(albums, 'No saved albums yet. Open an album and hit Save to library.')}
			</Tabs.Content>
			<Tabs.Content value="artists">
				{@render grid(artists, 'No artists yet. They show up once you save their songs or albums.')}
			</Tabs.Content>
		{/if}
	</Tabs.Root>
</div>
