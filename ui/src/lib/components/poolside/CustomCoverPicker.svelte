<!--
  CustomCoverPicker — a real modal that lets the user pick a custom label art for
  the vinyl disc on the Now/turntable view. Two rows:

    Top row: preset color swatches rendered as small disc previews (a red disc, a
    white disc, a black disc, etc.) so the user can pick a quick color without
    uploading anything.

    Bottom row: a grid of the user's own photos, each cropped into a circle to
    preview how it would look as the disc's center label. The user can pick a
    photo from their machine — we read it as a data URL and store it in
    localStorage (same key the previous modal used) so the disc reloads it
    everywhere it's shown.

  Selected art replaces the default album art for the cover when this disc is
  the "current" disc (art-for() in the shell picks covers[albumId] first, then the
  album's thumbnail).
-->
<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ImageAddIcon, Tick01Icon } from '@hugeicons/core-free-icons';
	import { fade, scale } from 'svelte/transition';
	import { onMount } from 'svelte';
	import type { BrowseItem } from '$lib/api';
	import { toast } from '$lib/player.svelte';

	let {
		album,
		open,
		covers,
		onClose,
		onChange
	}: {
		album: BrowseItem | null;
		open: boolean;
		covers: Record<string, string>;
		onClose: () => void;
		onChange: (covers: Record<string, string>) => void;
	} = $props();

	const PRESETS: { id: string; label: string; labelColor: string; artColor: string }[] = [
		{ id: 'css:red', label: 'RED', labelColor: '#1a1a1a', artColor: '#d6342b' },
		{ id: 'css:white', label: 'WHITE', labelColor: '#1a1a1a', artColor: '#f0f0f0' },
		{ id: 'css:black', label: 'BLACK', labelColor: '#f0f0f0', artColor: '#0a0a0a' },
		{ id: 'css:gold', label: 'GOLD', labelColor: '#3d2900', artColor: '#c89a3a' },
		{ id: 'css:sunset', label: 'SUNSET', labelColor: '#fff', artColor: 'linear-gradient(135deg,#ff6b3d,#ff9a3d)' },
		{ id: 'css:ocean', label: 'OCEAN', labelColor: '#fff', artColor: 'linear-gradient(135deg,#0c4a6e,#0ea5e9)' }
	];

	let userPhotos = $state<string[]>([]);
	let fileInput = $state<HTMLInputElement>();
	let activeKey = $state<string | null>(null);

	onMount(() => {
		// load user's previously-picked photos (separate from the per-album covers)
		try {
			const raw = localStorage.getItem('ps-user-photos');
			if (raw) userPhotos = JSON.parse(raw);
		} catch { /* quota */ }
	});

	function pick(albumId: string, art: string) {
		const next = { ...covers, [albumId]: art };
		activeKey = art;
		onChange(next);
		try { localStorage.setItem('ps-covers', JSON.stringify(next)); } catch { /* quota */ }
		toast.success('Cover applied to the disc');
	}

	function reset() {
		if (!album) return;
		const next = { ...covers };
		delete next[album.id];
		activeKey = null;
		onChange(next);
		try { localStorage.setItem('ps-covers', JSON.stringify(next)); } catch { /* quota */ }
		toast.info('Reset to printed art');
	}

	async function onFile(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const f = input.files?.[0];
		if (!f) return;
		const url = await new Promise<string>((res, rej) => {
			const rd = new FileReader();
			rd.onload = () => res(String(rd.result));
			rd.onerror = () => rej(rd.error);
			rd.readAsDataURL(f);
		});
		// Save to the user's photo bank (so it shows in the grid next time)
		const next = [url, ...userPhotos].slice(0, 24); // cap to 24 photos
		userPhotos = next;
		try { localStorage.setItem('ps-user-photos', JSON.stringify(next)); } catch { /* quota */ }
		// Apply to the current album
		if (album) pick(album.id, url);
		input.value = '';
	}

	function deleteUserPhoto(url: string, e: MouseEvent) {
		e.stopPropagation();
		userPhotos = userPhotos.filter((p) => p !== url);
		try { localStorage.setItem('ps-user-photos', JSON.stringify(userPhotos)); } catch { /* quota */ }
		toast.info('Photo removed');
	}

	$effect(() => {
		if (album) activeKey = covers[album.id] ?? null;
	});
</script>

{#if open && album}
	{@const a = album}
	<div
		class="ps-overlay open"
		role="dialog"
		aria-modal="true"
		aria-label="Add custom CD covers"
		tabindex="-1"
		onclick={(e) => {
			if (e.target === e.currentTarget) onClose();
		}}
		onkeydown={(e) => e.key === 'Escape' && onClose()}
		transition:fade={{ duration: 200 }}
	>
		<div class="ps-card text-center" in:scale={{ start: 0.96, duration: 260 }}>
			<button class="ps-cc-close" onclick={onClose} aria-label="Close">✕</button>
			<h2 class="serif-big">Add Custom CD Covers!</h2>
			<p class="sub">Print your own art onto the picture disc for "{a.title}"</p>

			<!-- Top row: preset color swatches rendered as small disc previews -->
			<div class="ps-cc-section-label">PRESET COLORS</div>
			<div class="ps-cc-opts">
				{#each PRESETS as p}
					<button
						class="ps-cc-opt {activeKey === p.id ? 'sel' : ''}"
						onclick={() => pick(a.id, p.id)}
						title={p.label}
					>
						<div class="ps-cc-disc" style="background: {p.artColor};">
							<span class="ps-cc-disc-label" style="color: {p.labelColor}">{p.label}</span>
						</div>
						{#if activeKey === p.id}
							<span class="ps-cc-tick"><HugeiconsIcon icon={Tick01Icon} /></span>
						{/if}
					</button>
				{/each}
			</div>

			<!-- Bottom row: the user's own photos as a grid of circular previews -->
			<div class="ps-cc-section-label">
				YOUR PHOTOS
				<button class="ps-cc-addphoto" onclick={() => fileInput?.click()} aria-label="Add a photo">
					<HugeiconsIcon icon={ImageAddIcon} />
					<span>Add Photo</span>
				</button>
			</div>
			<div class="ps-cc-photos">
				{#if userPhotos.length === 0}
					<div class="ps-cc-photos-empty">
						No photos yet. Use "Add Photo" to pick one from your computer.
					</div>
				{:else}
					{#each userPhotos as url}
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
						<div
							class="ps-cc-photo {activeKey === url ? 'sel' : ''}"
							role="button"
							tabindex="0"
							onclick={() => pick(a.id, url)}
							onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), pick(a.id, url))}
							title="Use this photo"
						>
							<img src={url} alt="" />
							{#if activeKey === url}
								<span class="ps-cc-tick"><HugeiconsIcon icon={Tick01Icon} /></span>
							{/if}
							<button
								class="ps-cc-photo-del"
								onclick={(e) => deleteUserPhoto(url, e)}
								aria-label="Remove photo"
							>✕</button>
						</div>
					{/each}
				{/if}
			</div>

			<input
				bind:this={fileInput}
				type="file"
				accept="image/*"
				hidden
				onchange={onFile}
			/>

			<div class="ps-cc-actions">
				<button class="ps-ghost" onclick={reset}>Reset to printed art</button>
				<button class="ps-aqua" onclick={onClose}>Done</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.ps-cc-close {
		all: unset;
		cursor: pointer;
		position: absolute;
		right: 18px;
		top: 14px;
		font-size: 18px;
		color: #fff;
		opacity: 0.7;
		transition: opacity 0.15s;
	}
	.ps-cc-close:hover { opacity: 1; }
	.ps-cc-section-label {
		display: flex;
		justify-content: space-between;
		align-items: center;
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.32em;
		text-transform: uppercase;
		opacity: 0.7;
		margin: 22px 0 12px;
	}
	.ps-cc-addphoto {
	all: unset;
	cursor: pointer;
	display: inline-flex;
	align-items: center;
	gap: 6px;
	padding: 4px 12px;
	border-radius: 999px;
	background: rgba(255, 255, 255, 0.15);
	border: 1px solid rgba(255, 255, 255, 0.25);
	font-size: 9px;
	letter-spacing: 0.18em;
	}
	.ps-cc-addphoto:hover { background: rgba(255, 255, 255, 0.25); }
	.ps-cc-opts {
		display: flex;
		justify-content: center;
		gap: 18px;
		flex-wrap: wrap;
	}
	.ps-cc-opt {
		all: unset;
		cursor: pointer;
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 6px;
	}
	.ps-cc-opt:hover .ps-cc-disc { transform: scale(1.06); }
	.ps-cc-opt.sel .ps-cc-disc {
		outline: 2px solid #fff;
		outline-offset: 4px;
	}
	.ps-cc-disc {
		width: 60px;
		height: 60px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		border: 2px solid rgba(0, 0, 0, 0.4);
		transition: transform 0.2s;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
	}
	.ps-cc-disc-label {
		font-size: 8px;
		font-weight: 700;
		letter-spacing: 0.2em;
	}
	.ps-cc-tick {
	position: absolute;
	top: -6px;
	right: -6px;
	width: 22px;
	height: 22px;
	border-radius: 50%;
	background: #4ade80;
	color: #062c1a;
	display: grid;
	place-items: center;
	box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
	}
	.ps-cc-photos {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(72px, 1fr));
		gap: 10px;
		max-width: 460px;
		margin: 0 auto;
	}
	.ps-cc-photos-empty {
		grid-column: 1 / -1;
		padding: 18px;
		font-size: 9px;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		opacity: 0.5;
		text-align: center;
	}
	.ps-cc-photo {
		all: unset;
		cursor: pointer;
		position: relative;
		aspect-ratio: 1;
		border-radius: 50%;
		overflow: hidden;
		border: 2px solid rgba(255, 255, 255, 0.3);
		transition: transform 0.2s, border-color 0.2s;
	}
	.ps-cc-photo:hover { transform: scale(1.06); }
	.ps-cc-photo.sel {
		border-color: #4ade80;
		border-width: 3px;
	}
	.ps-cc-photo img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}
	.ps-cc-photo-del {
		position: absolute;
		top: -4px;
		right: -4px;
		width: 20px;
		height: 20px;
		border-radius: 50%;
		background: rgba(0, 0, 0, 0.7);
		color: #fff;
		font-size: 11px;
		display: grid;
		place-items: center;
		opacity: 0;
		transition: opacity 0.15s;
	}
	.ps-cc-photo:hover .ps-cc-photo-del { opacity: 1; }
	.ps-cc-actions {
		display: flex;
		justify-content: center;
		gap: 12px;
		margin-top: 26px;
	}
</style>
