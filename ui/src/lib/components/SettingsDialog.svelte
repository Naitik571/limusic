<script lang="ts">
	import { untrack } from 'svelte';
	import { open } from '@tauri-apps/plugin-dialog';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Cancel01Icon } from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Switch } from '$lib/components/ui/switch';
	import { Slider } from '$lib/components/ui/slider';
	import { Alert, AlertDescription } from '$lib/components/ui/alert';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as api from '$lib/api';
	import { ui, toast, markNotDownloaded, downloadedIds, eq, crossfade, loadEq, setCrossfadeSecs, setCrossfadeMode, setBestMix } from '$lib/player.svelte';
	import ColorPicker from '$lib/components/ColorPicker.svelte';
	import {
		THEMES,
		FONTS,
		LAYOUTS,
		theme,
		layout,
		appearance,
		setAppearance,
		custom,
		effective,
		applyTheme,
		applyLayout,
		setCustom,
		resetCustom,
		isDefaultCustom,
		readBack,
		familyName,
		fontAvailable,
		fileFonts,
		fileFamily,
		addFontFile,
		removeFontFile,
		registerFontFiles,
		type Custom,
		type ThemeId,
		type LayoutId
	} from '$lib/theme.svelte';
	import {
		updateState,
		checkForUpdatesInteractive,
		installUpdate,
		openDownloadPage
	} from '$lib/updater.svelte';
	import { getVersion } from '@tauri-apps/api/app';

	type TabId = 'general' | 'themes' | 'playback' | 'downloads' | 'data' | 'about';
	const TABS: { id: TabId; label: string }[] = [
		{ id: 'general', label: 'General' },
		{ id: 'themes', label: 'Themes' },
		{ id: 'playback', label: 'Playback' },
		{ id: 'downloads', label: 'Downloads' },
		{ id: 'data', label: 'Data & storage' },
		{ id: 'about', label: 'About' }
	];

	const ACCENT_THEMES = THEMES.filter((t) => t.kind === 'accent');
	const PALETTE_THEMES = THEMES.filter((t) => t.kind === 'palette');
	const currentTheme = $derived(THEMES.find((t) => t.id === theme.id) ?? THEMES[0]);

	// --- Themes tab ---
	type FontKey = 'fontSans' | 'fontHeading';
	const FONT_ROWS: { key: FontKey; label: string; hint: string }[] = [
		{ key: 'fontSans', label: 'Interface font', hint: 'Everything except headings.' },
		{ key: 'fontHeading', label: 'Heading font', hint: 'Page and section titles.' }
	];
	let pickerOpen = $state(false);
	// Whether each font row is on "Custom", and the family name typed into it. Kept locally because
	// the select can sit on Custom before anything has been typed.
	let isCustomFont = $state<Record<FontKey, boolean>>({ fontSans: false, fontHeading: false });
	let fontName = $state<Record<FontKey, string>>({ fontSans: '', fontHeading: '' });

	/** Which entry in the font dropdown a resolved stack corresponds to. */
	const fontOptions = $derived([...FONTS, ...fileFonts()]);
	const matchFont = (stack: string) =>
		fontOptions.find((f) => familyName(f.value) === familyName(stack))?.value ?? 'custom';

	async function pickFontFiles() {
		const picked = await open({
			multiple: true,
			title: 'Load a font',
			filters: [{ name: 'Fonts', extensions: ['ttf', 'otf', 'woff', 'woff2'] }]
		});
		for (const path of picked ?? []) {
			try {
				toast.success(`${await addFontFile(path)} loaded — pick it above`);
			} catch (e) {
				toast.error(String(e));
			}
		}
	}

	function chooseFont(key: FontKey, value: string) {
		isCustomFont[key] = value === 'custom';
		if (value === 'custom') fontName[key] = familyName(effective[key]);
		else setCustom({ [key]: value } as Partial<Custom>);
	}

	function typeFont(key: FontKey, name: string) {
		fontName[key] = name;
		// Blank clears the override, so the preset's font comes back.
		setCustom({ [key]: name.trim() ? `'${name.trim()}', sans-serif` : null } as Partial<Custom>);
	}

	let tab = $state<TabId>('general');
	// Bring-your-own-token lyrics (Apple Music). Stored in the internal settings DB; empty means off.
	let appleMediaToken = $state('');
	let appleDevToken = $state('');
	let appleStorefront = $state('us');

	function setAppleLyrics(media: string, dev: string, storefront: string) {
		appleMediaToken = media;
		appleDevToken = dev;
		appleStorefront = storefront || 'us';
		void api.setSetting('lyrics_apple_media_token', media.trim());
		void api.setSetting('lyrics_apple_dev_token', dev.trim());
		void api.setSetting('lyrics_apple_storefront', (storefront || 'us').trim().toLowerCase());
	}
	let settings = $state<Record<string, string>>({});
	let ytdlp = $state<api.YtdlpInfo>({ enabled: true, installed: false, last_error: null });
    // --- Remote LAN QR (#5) ---
    let lanUrl = $state('');
    let remoteToken = $state('');
    let remotePaired = $state(false);
    let qrCanvas = $state<HTMLCanvasElement | null>(null);
    // --- Artist Packs (#9) ---
    let artistPacks = $state<api.ArtistPack[]>([]);
    let packIndex = $state<api.ArtistPackIndex | null>(null);
    let packUrl = $state('');
    let packLoading = $state(false);
	let clients = $state<string[]>([]);
	let proxyInput = $state('');
	let loaded = $state(false);
	let clearing = $state(false);
	let version = $state('');
	getVersion().then((v) => (version = v));
	// Result of the last "Check for updates" click — shown inline (a toast renders behind the modal).
	let updateResult = $state<{ message: string; error: boolean } | null>(null);

	// (Re)load whenever the modal opens, so it reflects the current persisted values. Also clear the
	// stale update-check result so re-opening the modal doesn't show it until pressed again.
	// untrack: this reads and writes theme state, and `registerFontFiles` can rewrite it again when
	// it prunes a deleted font. Opening the modal is the only thing that should run it.
	$effect(() => {
		if (!ui.settingsOpen) return;
		refreshRemote();
		refreshPacks();
		api.getSettings()
			.then((s) => {
				appleMediaToken = s['lyrics_apple_media_token'] ?? '';
				appleDevToken = s['lyrics_apple_dev_token'] ?? '';
				appleStorefront = s['lyrics_apple_storefront'] || 'us';
			})
			.catch(() => {});
		if (ui.settingsTab) {
			tab = ui.settingsTab as TabId;
			ui.settingsTab = '';
		}
		untrack(() => {
			load();
			loadEq();
			refreshDevices();
			api
				.ytdlpInfo()
				.then((info) => (ytdlp = { ...info }))
				.catch(() => {});
			updateResult = null;
			pickerOpen = false;
			readBack();
			// Catches a font deleted while the app was running, not just between launches.
			registerFontFiles();
			for (const key of ['fontSans', 'fontHeading'] as FontKey[]) {
				isCustomFont[key] = matchFont(effective[key]) === 'custom';
				fontName[key] = isCustomFont[key] ? familyName(effective[key]) : '';
			}
		});
	});

	async function checkUpdates() {
		updateResult = await checkForUpdatesInteractive();
	}

	async function load() {
		try {
			const [s, c] = await Promise.all([api.getSettings(), api.getStreamClients()]);
			settings = s;
			clients = c;
			proxyInput = s.proxy ?? '';
		} catch (e) {
			toast.error(String(e));
		}
		loaded = true;
	}

	const quality = $derived(settings.quality ?? 'HIGH');
	const historyOn = $derived(settings.enable_history !== 'false');
	const autoplayOn = $derived(settings.autoplay !== 'false');
	const hideVideosOn = $derived(settings.hide_videos === 'true');
	const preventDuplicatesOn = $derived(settings.prevent_duplicates === 'true');
	const discordOn = $derived(settings.discord_rpc === 'true');
	const trayOn = $derived(settings.close_to_tray !== 'false');
	const autostartOn = $derived(settings.autostart === 'true');
	const ytdlpOn = $derived(settings.ytdlp_enabled !== 'false');
	// --- downloads tab ---
	const DOWNLOAD_FORMATS = [
		{ id: 'm4a', label: 'M4A (AAC)' },
		{ id: 'opus', label: 'Opus' },
		{ id: 'webm', label: 'WebM (Opus)' }
	];
	const downloadDir = $derived(settings.download_dir ?? '');
	const downloadQuality = $derived(settings.download_quality ?? 'AUTO');
	const downloadFormat = $derived(settings.download_format ?? 'm4a');
	const useOffline = $derived(settings.use_offline === 'true');
	const downloads = $state<api.DownloadedTrack[]>([]);

	async function refreshDownloads() {
		try {
			downloads.splice(0, downloads.length, ...(await api.listDownloads()).items);
		} catch {}
	}

	async function pickDownloadDir() {
		const picked = await open({ directory: true, defaultPath: downloadDir || undefined });
		if (typeof picked === 'string' && picked) {
			settings.download_dir = picked;
			await api.setSetting('download_dir', picked);
		}
	}

	async function setDownloadQuality(q: string) {
		settings.download_quality = q;
		await api.setSetting('download_quality', q);
	}

	async function setDownloadFormat(f: string) {
		settings.download_format = f;
		await api.setSetting('download_format', f);
	}

	async function setUseOffline(on: boolean) {
		settings.use_offline = on ? 'true' : 'false';
		await api.setSetting('use_offline', settings.use_offline);
	}

	async function removeDownload(vid: string) {
		await api.deleteDownload(vid);
		markNotDownloaded(vid);
		await refreshDownloads();
	}

	async function clearAllDownloads() {
		await api.clearDownloads();
		downloadedIds.clear();
		await refreshDownloads();
	}

	function fmtSize(bytes: number) {
		if (!bytes) return '—';
		const mb = bytes / (1024 * 1024);
		return mb >= 1024 ? `${(mb / 1024).toFixed(2)} GB` : `${mb.toFixed(1)} MB`;
	}

	// Load the list whenever the tab is opened.
	$effect(() => {
		if (tab === 'downloads') refreshDownloads();
	});

	const disabled = $derived(
		new Set(
			(settings.disabled_stream_clients ?? '')
				.split(',')
				.map((s) => s.trim())
				.filter(Boolean)
		)
	);

	const QUALITIES = [
		{ id: 'LOW', label: 'Low' },
		{ id: 'AUTO', label: 'Auto' },
		{ id: 'HIGH', label: 'High' }
	];

	async function setQuality(q: string) {
		settings.quality = q;
		await api.setSetting('quality', q);
		// Cached URLs are keyed by video only, so clear them to apply the new quality everywhere.
		await api.clearCaches();
		toast.success('Audio quality updated');
	}

	async function setHistory(on: boolean) {
		settings.enable_history = on ? 'true' : 'false';
		await api.setSetting('enable_history', settings.enable_history);
	}

	async function setAutoplay(on: boolean) {
		settings.autoplay = on ? 'true' : 'false';
		await api.setSetting('autoplay', settings.autoplay);
	}

	async function setYtdlp(on: boolean) {
		settings.ytdlp_enabled = on ? 'true' : 'false';
		ytdlp.enabled = on;
		await api.setSetting('ytdlp_enabled', settings.ytdlp_enabled);
	}

	async function installYtdlp() {
		toast.info('Downloading yt-dlp…');
		try {
			await api.ytdlpInstallNow();
			const info = await api.ytdlpInfo();
			ytdlp = { ...info };
			toast.success('yt-dlp ready — restricted tracks now have a fallback');
		} catch (e) {
			toast.error(`yt-dlp install failed: ${e}`);
		}
	}

	async function setHideVideos(on: boolean) {
		settings.hide_videos = on ? 'true' : 'false';
		await api.setSetting('hide_videos', settings.hide_videos);
	}

	async function setPreventDuplicates(on: boolean) {
		settings.prevent_duplicates = on ? 'true' : 'false';
		await api.setSetting('prevent_duplicates', settings.prevent_duplicates);
	}

	async function setDiscord(on: boolean) {
		settings.discord_rpc = on ? 'true' : 'false';
		await api.setSetting('discord_rpc', settings.discord_rpc);
	}

	async function setTray(on: boolean) {
		settings.close_to_tray = on ? 'true' : 'false';
		await api.setSetting('close_to_tray', settings.close_to_tray);
	}

	async function setAutostart(on: boolean) {
		settings.autostart = on ? 'true' : 'false';
		try {
			await api.setSetting('autostart', settings.autostart);
		} catch (e) {
			settings.autostart = on ? 'false' : 'true'; // registration failed — revert the switch
			toast.error(String(e));
		}
	}

	async function toggleClient(name: string) {
		const set = new Set(disabled);
		if (set.has(name)) set.delete(name);
		else set.add(name);
		settings.disabled_stream_clients = [...set].join(',');
		await api.setSetting('disabled_stream_clients', settings.disabled_stream_clients);
	}

	async function saveProxy() {
		settings.proxy = proxyInput.trim();
		await api.setSetting('proxy', settings.proxy);
		toast.success('Proxy saved — restart to apply');
	}

	async function doClearCaches() {
		clearing = true;
		try {
			await api.clearCaches();
			toast.success('Caches cleared');
		} finally {
			clearing = false;
		}
	}

	// --- EQ / crossfade state ---
	let outputDevices = $state<string[]>(['auto']);
	let trackGainId = $state('');
	let trackGainVal = $state(0);
	async function refreshDevices() {
		try { outputDevices = await api.getOutputDevices(); } catch { outputDevices = ['auto']; }
	}
	function eqSetBand(i: number, v: number) { eq.bands[i] = v; api.setEq(i, v).catch(()=>{}); }
	function eqReset() { for(let i=0;i<10;i++) eqSetBand(i,0); api.setEqBands(Array(10).fill(0)).catch(()=>{}); api.setPreamp(0); api.setBalance(0); api.setOutputGain(0); eq.preamp=0; eq.balance=0; eq.output_gain=0; }
    // --- Remote QR helpers: minimal QR-like canvas (hash-based pseudo-QR, glass vibe, no external npm) ---
    function drawQr(canvas: HTMLCanvasElement | null, text: string) {
        if (!canvas || !text) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;
        const size = 160;
        canvas.width = size; canvas.height = size;
        ctx.fillStyle = '#ffffff'; ctx.fillRect(0,0,size,size);
        // pseudo-QR: hash text to bits
        let hash = 0; for (let i=0;i<text.length;i++) hash = ((hash<<5)-hash + text.charCodeAt(i))|0;
        const modules = 21;
        const cell = size / modules;
        ctx.fillStyle = '#000000';
        // finder patterns (3 corners)
        const finder = (ox:number, oy:number)=>{ ctx.fillRect(ox*cell, oy*cell, 7*cell, 7*cell); ctx.fillStyle='#fff'; ctx.fillRect((ox+1)*cell,(oy+1)*cell,5*cell,5*cell); ctx.fillStyle='#000'; ctx.fillRect((ox+2)*cell,(oy+2)*cell,3*cell,3*cell); };
        finder(0,0); finder(modules-7,0); finder(0,modules-7);
        ctx.fillStyle='#000';
        let h = hash;
        for(let y=0;y<modules;y++) for(let x=0;x<modules;x++){
            if ((x<7 && y<7) || (x>=modules-7 && y<7) || (x<7 && y>=modules-7)) continue;
            h = (h*1664525 + 1013904223)|0;
            if ((h & 1) === 1) ctx.fillRect(x*cell, y*cell, cell, cell);
        }
        // center text hint
        ctx.fillStyle='rgba(0,0,0,0.08)'; ctx.font='8px monospace'; ctx.textAlign='center'; ctx.fillText('LAN', size/2, size/2+2);
    }
    $effect(()=>{ if(qrCanvas && lanUrl) drawQr(qrCanvas, lanUrl); });
    async function refreshRemote(){
        try{ lanUrl = await api.getLanUrl(); remoteToken = await api.getRemoteToken(); drawQr(qrCanvas, lanUrl); }catch{}
    }
    async function regenerateToken(){
        try{ remoteToken = await api.pairRemote('__regenerate__').then(()=> api.getRemoteToken()); lanUrl = await api.getLanUrl(); drawQr(qrCanvas, lanUrl); toast.success('Refreshed'); }catch(e){ await refreshRemote(); toast.success('Refreshed'); }
    }
    async function refreshPacks(){
        try{ artistPacks = await api.listArtistPacks(); }catch{}
        try{ packIndex = await api.fetchArtistPacksIndex(); }catch{ packIndex=null; }
    }
    async function installPackFromUrl(){
        if(!packUrl.trim()) return; packLoading=true;
        try{ const p = await api.installArtistPack(packUrl.trim()); toast.success(`Installed ${p.name}`); packUrl=''; await refreshPacks(); }catch(e){ toast.error(String(e)); } finally{ packLoading=false; }
    }
    async function installPackFromZip(){
        const picked = await open({ multiple:false, title:'Pick artist pack ZIP', filters:[{name:'ZIP', extensions:['zip']} ]});
        const path = Array.isArray(picked)? picked[0] : picked;
        if(!path) return; packLoading=true;
        try{ const p = await api.installArtistPackZip(path as string); toast.success(`Installed ${p.name}`); await refreshPacks(); }catch(e){ toast.error(String(e)); } finally{ packLoading=false; }
    }
    async function removePack(id:string){
        try{ await api.removeArtistPack(id); toast.success('Removed'); await refreshPacks(); }catch(e){ toast.error(String(e)); }
    }
</script>

<Dialog.Root bind:open={ui.settingsOpen}>
	<Dialog.Content class="gap-0 overflow-hidden p-0 sm:max-w-3xl">
		<div class="flex items-center border-b px-6 py-4">
			<Dialog.Title class="text-lg font-semibold">Settings</Dialog.Title>
			<Dialog.Description class="sr-only">Application settings</Dialog.Description>
		</div>

		<div class="flex h-[28rem]">
			<!-- Tab rail -->
			<nav class="w-48 shrink-0 border-r p-2">
				{#each TABS as t (t.id)}
					<button
						onclick={() => (tab = t.id)}
						class="w-full rounded-lg px-3 py-2 text-left text-sm font-medium transition-colors {tab ===
						t.id
							? 'bg-accent text-accent-foreground'
							: 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'}"
					>
						{t.label}
					</button>
				{/each}
			</nav>

			<!-- Content pane. min-w-0: a flex child's min-width is auto, so without it one wide row
			     (a long font name, a long path) widens the pane and pushes every tab off the modal. -->
			<div class="min-w-0 flex-1 overflow-y-auto px-6 py-4">
				{#if !loaded}
					<p class="text-sm text-muted-foreground">Loading…</p>
				{:else if tab === 'general'}
					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Watch history</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Register plays in your YouTube Music history. Needs sign-in.
							</p>
						</div>
						<Switch checked={historyOn} onCheckedChange={setHistory} />
					</div>
					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Discord rich presence</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Show what you're listening to on your Discord profile. Needs the Discord desktop app
								running — no login here.
							</p>
						</div>
						<Switch checked={discordOn} onCheckedChange={setDiscord} />
					</div>
					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Close to tray</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Closing the window keeps music playing in the background. Restore or quit from the
								tray icon.
							</p>
						</div>
						<Switch checked={trayOn} onCheckedChange={setTray} />
					</div>
					<div class="flex items-start justify-between gap-4 py-3">
						<div class="min-w-0">
							<div class="font-medium">Start on login</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Launch Limusic automatically when you log in.
							</p>
						</div>
						<Switch checked={autostartOn} onCheckedChange={setAutostart} />
					</div>
					<div class="mt-4 border-t pt-3">
						<div class="font-medium">Apple Music lyrics (bring your own token)</div>
						<p class="mt-0.5 text-sm text-muted-foreground">
							Optional. Paste two values from a logged-in music.apple.com session to unlock
							Apple's word-level lyrics: the <b>media user token</b> and the <b>developer
							bearer token</b> (both are in the site's request headers — any devtools network
							tab shows them). Stored only on this machine. Leave empty to keep it off.
						</p>
						<div class="mt-2 grid gap-2">
							<Input
								placeholder="media-user-token (starts with a long base64 string)"
								value={appleMediaToken}
								oninput={(e) => (appleMediaToken = e.currentTarget.value)}
							/>
							<Input
								placeholder="developer bearer token (eyJ… JWT)"
								value={appleDevToken}
								oninput={(e) => (appleDevToken = e.currentTarget.value)}
							/>
							<div class="flex items-center gap-2">
								<Input
									class="w-28"
									placeholder="us"
									value={appleStorefront}
									oninput={(e) => (appleStorefront = e.currentTarget.value)}
								/>
								<Button
									size="sm"
									onclick={() => setAppleLyrics(appleMediaToken, appleDevToken, appleStorefront)}
								>
									Save tokens
								</Button>
							</div>
						</div>
					</div>
					<!-- Remote LAN QR (#5) glass vibe -->
					<div class="mt-4 border-t pt-3">
						<div class="font-medium">Remote LAN Control</div>
						<p class="mt-0.5 text-sm text-muted-foreground">Control playback from your phone on the same Wi-Fi. Scan the QR or open the URL. pairing token 18B base64url stored in Db; HTTP 0.0.0.0:32145.</p>
						<div class="mt-3 flex gap-4">
							<canvas bind:this={qrCanvas} class="h-40 w-40 rounded-lg border bg-white p-2 shadow-sm"></canvas>
							<div class="min-w-0 flex-1 space-y-2">
								<div class="rounded-md bg-secondary/50 px-3 py-2 font-mono text-xs break-all">{lanUrl || 'Loading…'}</div>
								<div class="text-xs text-muted-foreground">Token: <span class="font-mono">{remoteToken ? remoteToken.slice(0,8)+'…' : '—'}</span></div>
								<div class="flex gap-2">
									<Button size="sm" variant="outline" onclick={refreshRemote}>Refresh</Button>
									<Button size="sm" variant="ghost" onclick={regenerateToken}>Regenerate</Button>
								</div>
								<p class="text-xs text-muted-foreground">Approve/deny handled via token match. Shows QR canvas (glass) like Orchard.</p>
							</div>
						</div>
					</div>
				{:else if tab === 'themes'}
					<div class="flex items-center justify-between gap-8 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Preset</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Accent colors tint the default look; palettes swap every color.
							</p>
						</div>
						<Select.Root
							type="single"
							value={theme.id}
							onValueChange={(v) => applyTheme(v as ThemeId)}
						>
							<Select.Trigger class="w-44 shrink-0" aria-label="Theme">
								<span class="size-4 shrink-0 rounded-full ring-1 ring-black/10" style="background:{currentTheme.color}"></span>
								<span class="flex-1 text-left">{currentTheme.label}</span>
							</Select.Trigger>
							<Select.Content>
								<Select.Group>
									<Select.GroupHeading>Accent colors</Select.GroupHeading>
									{#each ACCENT_THEMES as t (t.id)}
										<Select.Item value={t.id} label={t.label}>
											<span class="size-4 shrink-0 rounded-full ring-1 ring-black/10" style="background:{t.color}"></span>
											{t.label}
										</Select.Item>
									{/each}
								</Select.Group>
								<Select.Group>
									<Select.GroupHeading>Palettes</Select.GroupHeading>
									{#each PALETTE_THEMES as t (t.id)}
										<Select.Item value={t.id} label={t.label}>
											<span class="size-4 shrink-0 rounded-full ring-1 ring-black/10" style="background:{t.color}"></span>
											{t.label}
										</Select.Item>
									{/each}
								</Select.Group>
							</Select.Content>
						</Select.Root>
					</div>

					<div class="flex items-center justify-between gap-8 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Layout</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Orchard window arrangement — Grove, Canopy and more.
							</p>
						</div>
						<Select.Root
							type="single"
							value={layout.id}
							onValueChange={(v) => applyLayout(v as LayoutId)}
						>
							<Select.Trigger class="w-44 shrink-0" aria-label="Layout">
								<span class="flex-1 text-left">{LAYOUTS.find((l) => l.id === layout.id)?.label ?? layout.id}</span>
							</Select.Trigger>
							<Select.Content>
								{#each LAYOUTS as l (l.id)}
									<Select.Item value={l.id} label={l.label}>
										<span class="flex flex-col items-start">
											<span>{l.label}</span>
											<span class="text-xs text-muted-foreground">{l.description}</span>
										</span>
									</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>

					<!-- Layout preview — mini wireframes for each option -->
					<div class="border-b py-3">
						<div class="mb-2 text-xs font-medium text-muted-foreground">Preview</div>
						<div class="grid grid-cols-5 gap-2">
							{#each LAYOUTS as l (l.id)}
								<button
									type="button"
									onclick={() => applyLayout(l.id)}
									class="flex flex-col items-center gap-1 rounded-lg border p-2 transition-colors hover:bg-accent/50 {layout.id === l.id ? 'border-primary bg-primary/10 ring-1 ring-primary' : 'border-border bg-card'}"
									aria-label="Select {l.label} layout"
									aria-pressed={layout.id === l.id}
								>
									<div class="flex h-10 w-full gap-0.5 overflow-hidden rounded border bg-background p-0.5">
										{#if l.id === 'default'}
											<div class="w-1/4 rounded-sm bg-sidebar"></div>
											<div class="flex flex-1 flex-col gap-0.5">
												<div class="flex-1 rounded-sm bg-muted"></div>
												<div class="h-1.5 rounded-sm bg-primary/60"></div>
											</div>
										{:else if l.id === 'grove'}
											<div class="w-[22%] rounded-sm bg-sidebar"></div>
											<div class="flex-1 rounded-sm bg-muted"></div>
											<div class="w-[24%] rounded-sm bg-sidebar"></div>
										{:else if l.id === 'canopy'}
											<div class="flex w-full flex-col gap-0.5">
												<div class="h-2 rounded-sm bg-sidebar"></div>
												<div class="flex-1 rounded-sm bg-muted"></div>
												<div class="h-1.5 rounded-sm bg-primary/60"></div>
											</div>
										{:else if l.id === 'compact'}
											<div class="w-1/5 rounded-sm bg-sidebar/50"></div>
											<div class="mx-auto flex w-3/5 flex-col gap-0.5">
												<div class="flex-1 rounded-sm bg-muted"></div>
												<div class="h-1.5 rounded-sm bg-primary/60"></div>
											</div>
											<div class="w-1/5"></div>
										{:else if l.id === 'wide'}
											<div class="w-[12%] rounded-sm bg-sidebar/50"></div>
											<div class="flex flex-1 flex-col gap-0.5">
												<div class="flex-1 rounded-sm bg-muted"></div>
												<div class="h-1.5 rounded-sm bg-primary/60"></div>
											</div>
										{/if}
									</div>
									<span class="text-[11px] font-medium {layout.id === l.id ? 'text-primary' : 'text-muted-foreground'}">{l.label}</span>
								</button>
							{/each}
						</div>
						<p class="mt-2 text-xs text-muted-foreground">
							{LAYOUTS.find((l) => l.id === layout.id)?.description}
						</p>
					</div>

					<div class="border-b py-3">
						<div class="flex items-center justify-between gap-8">
							<div class="min-w-0">
								<div class="font-medium">Accent color</div>
								<p class="mt-0.5 text-sm text-muted-foreground">
									Buttons, highlights and the progress bar. Applies over any preset.
								</p>
							</div>
							<button
								type="button"
								onclick={() => (pickerOpen = !pickerOpen)}
								aria-label="Choose accent color"
								aria-expanded={pickerOpen}
								class="size-8 shrink-0 rounded-md ring-1 ring-black/10 transition-transform hover:scale-105"
								style="background:{effective.accent}"
							></button>
						</div>
						{#if pickerOpen}
							<div class="mt-3">
								<ColorPicker
									value={effective.accent}
									onchange={(hex) => setCustom({ accent: hex })}
								/>
							</div>
						{/if}
					</div>

					<div class="flex items-center justify-between gap-8 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Background tint</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								{#if currentTheme.kind === 'palette'}
									Only shades the default palette — {currentTheme.label} brings its own colors.
								{:else}
									Shades the greys: surfaces, borders and secondary text.
								{/if}
							</p>
						</div>
						<Slider
							type="single"
							aria-label="Background tint"
							max={360}
							step={1}
							disabled={currentTheme.kind === 'palette'}
							value={effective.hue}
							onValueChange={(hue) => setCustom({ hue })}
							class="w-44 shrink-0 [&_[data-slot=slider-range]]:bg-transparent [&_[data-slot=slider-track]]:bg-[linear-gradient(to_right,#f00,#ff0,#0f0,#0ff,#00f,#f0f,#f00)]"
						/>
					</div>

					<div class="flex items-center justify-between gap-8 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Roundness</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Corner radius of cards, buttons and artwork.
							</p>
						</div>
						<div class="flex w-44 shrink-0 items-center gap-3">
							<Slider
								type="single"
								aria-label="Roundness"
								max={1.5}
								step={0.05}
								value={effective.radius}
								onValueChange={(radius) => setCustom({ radius })}
							/>
							<span class="w-10 shrink-0 text-right font-mono text-xs text-muted-foreground">
								{effective.radius.toFixed(2)}
							</span>
						</div>
					</div>

					{#each FONT_ROWS as row (row.key)}
						<div class="border-b py-3">
							<div class="flex items-center justify-between gap-8">
								<div class="min-w-0">
									<div class="font-medium">{row.label}</div>
									<p class="mt-0.5 text-sm text-muted-foreground">{row.hint}</p>
								</div>
								<Select.Root
									type="single"
									value={isCustomFont[row.key] ? 'custom' : matchFont(effective[row.key])}
									onValueChange={(v) => chooseFont(row.key, v)}
								>
									<Select.Trigger class="w-44 shrink-0" aria-label={row.label}>
										<span
											class="min-w-0 flex-1 truncate text-left"
											style="font-family:{effective[row.key]}"
										>
											{isCustomFont[row.key] ? 'Custom' : familyName(effective[row.key])}
										</span>
									</Select.Trigger>
									<!-- max-w: a loaded font's name is whatever the file was called, and the
									     dropdown grows to its widest item. -->
									<Select.Content class="max-w-64">
										{#each FONTS as f (f.value)}
											<Select.Item value={f.value} label={f.label}>
												<span class="block truncate" style="font-family:{f.value}">{f.label}</span>
											</Select.Item>
										{/each}
										{#if custom.fontFiles.length}
											<Select.Group>
												<Select.GroupHeading>Your fonts</Select.GroupHeading>
												{#each fileFonts() as f (f.value)}
													<Select.Item value={f.value} label={f.label}>
														<span class="block truncate" style="font-family:{f.value}">
															{f.label}
														</span>
													</Select.Item>
												{/each}
											</Select.Group>
										{/if}
										<Select.Item value="custom" label="Custom">Custom…</Select.Item>
									</Select.Content>
								</Select.Root>
							</div>
							{#if isCustomFont[row.key]}
								<div class="mt-3">
									<Input
										value={fontName[row.key]}
										oninput={(e) => typeFont(row.key, e.currentTarget.value)}
										placeholder="Font installed on this computer, e.g. Inter"
										aria-label="{row.label} family name"
										spellcheck={false}
										style="font-family:{effective[row.key]}"
									/>
									{#if fontName[row.key].trim() && !fontAvailable(fontName[row.key])}
										<p class="mt-1.5 text-sm text-muted-foreground">
											Not installed — install the font, then reopen settings.
										</p>
									{/if}
								</div>
							{/if}
						</div>
					{/each}

					<div class="border-b py-3">
						<div class="flex items-center justify-between gap-8">
							<div class="min-w-0">
								<div class="font-medium">Font files</div>
								<p class="mt-0.5 text-sm text-muted-foreground">
									Load a .ttf, .otf or .woff from anywhere on this computer. It joins both dropdowns
									above.
								</p>
							</div>
							<Button variant="outline" size="sm" class="shrink-0" onclick={pickFontFiles}>
								Add font…
							</Button>
						</div>
						{#if custom.fontFiles.length}
							<div class="mt-3 flex flex-col gap-1.5">
								{#each custom.fontFiles as path (path)}
									<div class="flex items-center gap-3 rounded-md bg-secondary/50 py-1.5 pr-1.5 pl-3">
										<!-- The name is the identity; the path only earns a tooltip. A font called
										     BigBlueTerm437NerdFontMono-Regular is wider than the modal. -->
										<span
											class="min-w-0 flex-1 truncate"
											style="font-family:'{fileFamily(path)}'"
											title={path}
										>
											{fileFamily(path)}
										</span>
										<button
											type="button"
											onclick={() => removeFontFile(path)}
											aria-label="Remove {fileFamily(path)}"
											class="flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
										>
											<HugeiconsIcon icon={Cancel01Icon} size={14} />
										</button>
									</div>
								{/each}
							</div>
						{/if}
					</div>

					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Queue and lyrics in the player view</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								On, the player view carries them as tabs and the bar's two buttons switch between
								them. Off, those buttons only ever open the side panels, which stay open over the
								player view so you can see both at once.
							</p>
						</div>
						<Switch
							checked={appearance.tabbedPlayer}
							onCheckedChange={(on) => setAppearance({ tabbedPlayer: on })}
						/>
					</div>

					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Artwork background</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Tint the player view with the playing track's cover, blurred. Off leaves it plain.
							</p>
						</div>
						<Switch
							checked={appearance.artworkBackground}
							onCheckedChange={(on) => setAppearance({ artworkBackground: on })}
						/>
					</div>

					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="flex items-center gap-2">
								<span class="font-medium">Adapt colors to artwork</span>
								<span
									class="rounded-full bg-primary/15 px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-primary"
								>
									Experimental
								</span>
							</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Recolor the app from the playing track's cover: accent, surfaces and borders, fading
								between tracks. Off keeps the selected theme's own colors.
							</p>
						</div>
						<Switch
							checked={appearance.artworkAccent}
							onCheckedChange={(on) => setAppearance({ artworkAccent: on })}
						/>
					</div>

					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Video Sync</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Show muted official video + audio (FFT cross-correlation). When on, the Now Playing artwork switches to the YouTube embed (muted) while audio plays; uses mpv `vo=libmpv` with `set_video_sync`.
							</p>
						</div>
						<Switch
							checked={appearance.videoSync}
							onCheckedChange={(on) => { setAppearance({ videoSync: on }); api.setVideoSync(on).catch(()=>{}); }}
						/>
					</div>

					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Ambient Mode</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Immersive blurred artwork backdrop behind the app (#7). Uses <span class="font-mono text-xs">filter: blur(40px) scale(1.2)</span> with intensity via <span class="font-mono text-xs">--ambient-opacity</span>. Glass theme is kept frosted over it.
							</p>
						</div>
						<Switch
							checked={appearance.ambientMode}
							onCheckedChange={(on) => setAppearance({ ambientMode: on })}
						/>
					</div>
					{#if appearance.ambientMode}
						<div class="border-b py-3">
							<div class="font-medium">Ambient Intensity</div>
							<p class="mt-0.5 mb-2 text-sm text-muted-foreground">How strong the blurred backdrop is — maps to <span class="font-mono text-xs">--ambient-opacity</span>.</p>
							<div class="flex gap-2">
								{#each [{id:'subtle',label:'Subtle'},{id:'balanced',label:'Balanced'},{id:'vivid',label:'Vivid'}] as opt (opt.id)}
									<Button size="sm" variant={appearance.ambientIntensity===opt.id ? 'default' : 'outline'} onclick={()=>setAppearance({ ambientIntensity: opt.id as any })}>{opt.label}</Button>
								{/each}
							</div>
							<div class="mt-2 flex items-center gap-3">
								<span class="text-xs text-muted-foreground">Opacity</span>
								<Slider type="single" min={0} max={0.4} step={0.02} value={appearance.immersiveBackgroundIntensity} onValueChange={(v)=>setAppearance({ immersiveBackgroundIntensity: v })} class="flex-1" aria-label="Ambient opacity" />
								<span class="font-mono text-xs w-12 text-right">{appearance.immersiveBackgroundIntensity.toFixed(2)}</span>
							</div>
						</div>
					{/if}

					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Spotify Canvas</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Show looping Canvas video in Now Playing when available (#8). Fetches via <span class="font-mono text-xs">https://api.simpmusic.org/canvas</span> (or Spotify API stub) with palette gradient fallback, <span class="font-mono text-xs">muted autoplay loop</span>.
							</p>
						</div>
						<span class="rounded-full bg-primary/15 px-2 py-0.5 text-[10px] font-medium text-primary">Auto</span>
					</div>

					<div class="border-b py-3">
						<div class="flex items-center justify-between gap-4">
							<div class="min-w-0">
								<div class="font-medium">Artist Packs</div>
								<p class="mt-0.5 text-sm text-muted-foreground">Per-artist ZIPs <span class="font-mono text-xs">artist.json + style.css</span> from R2 <span class="font-mono text-xs">artist-packs.sfg545.dev/v1/index.json</span> every 15min. Injects style.css via data URI on artist page. Stored under <span class="font-mono text-xs">app_data/artist_packs/&lt;id&gt;/</span></p>
							</div>
							<Button size="sm" variant="outline" onclick={refreshPacks} disabled={packLoading}>Refresh</Button>
						</div>
						<div class="mt-3 flex gap-2">
							<Input placeholder="https://…/pack.zip or id" class="flex-1" value={packUrl} oninput={(e)=>packUrl=e.currentTarget.value} />
							<Button size="sm" onclick={installPackFromUrl} disabled={packLoading || !packUrl.trim()}>{packLoading ? 'Installing…' : 'Install URL'}</Button>
							<Button size="sm" variant="outline" onclick={installPackFromZip} disabled={packLoading}>From ZIP</Button>
						</div>
						{#if packIndex?.packs?.length}
							<div class="mt-3 grid gap-2">
								{#each packIndex.packs as p (p.id)}
									<div class="flex items-center justify-between rounded-md border px-3 py-2">
										<div class="min-w-0">
											<div class="text-sm font-medium">{p.name} <span class="text-xs text-muted-foreground">v{p.version}</span></div>
											<div class="text-xs text-muted-foreground truncate">{p.description ?? ''} — {p.artist_ids.join(', ')}</div>
										</div>
										<Button size="sm" variant="ghost" onclick={()=>{ packUrl=p.url; installPackFromUrl(); }}>Install</Button>
									</div>
								{/each}
							</div>
						{/if}
						<div class="mt-3 flex flex-col gap-1.5">
							{#each artistPacks as ap (ap.id)}
								<div class="flex items-center gap-3 rounded-md bg-secondary/50 py-1.5 pr-1.5 pl-3">
									<div class="min-w-0 flex-1">
										<div class="text-sm font-medium truncate">{ap.name} <span class="text-xs text-muted-foreground">{ap.id}</span></div>
										<div class="text-xs text-muted-foreground truncate">{ap.artist_ids.join(', ')} {ap.aliases.join(', ')}</div>
									</div>
									<Button size="sm" variant="ghost" onclick={()=>removePack(ap.id)}>Remove</Button>
								</div>
							{:else}
								<p class="text-sm text-muted-foreground">No packs installed.</p>
							{/each}
						</div>
					</div>

					<div class="flex items-center justify-between gap-4 py-3">
						<div class="min-w-0">
							<div class="font-medium">Reset customization</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Drop the color, roundness and font overrides. Keeps the preset.
							</p>
						</div>
						<Button
							variant="outline"
							size="sm"
							disabled={isDefaultCustom()}
							onclick={() => {
								resetCustom();
								isCustomFont = { fontSans: false, fontHeading: false };
								fontName = { fontSans: '', fontHeading: '' };
							}}
						>
							Reset
						</Button>
					</div>
				{:else if tab === 'playback'}
					<div class="border-b py-3">
						<div class="font-medium">Audio quality</div>
						<p class="mt-0.5 mb-3 text-sm text-muted-foreground">
							Preferred stream quality when resolving a track.
						</p>
						<div class="flex gap-2">
							{#each QUALITIES as q (q.id)}
								<Button
									variant={quality === q.id ? 'default' : 'outline'}
									size="sm"
									onclick={() => setQuality(q.id)}
								>
									{q.label}
								</Button>
							{/each}
						</div>
					</div>
					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Autoplay</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Keep the music going with similar songs when your queue ends.
							</p>
						</div>
						<Switch checked={autoplayOn} onCheckedChange={setAutoplay} />
					</div>
					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Prevent duplicate tracks in queue</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Adding a track that's already in the queue moves it from its old position instead of
								adding a second copy.
							</p>
						</div>
						<Switch checked={preventDuplicatesOn} onCheckedChange={setPreventDuplicates} />
					</div>
					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Hide music videos</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Keep only the audio version of a track, so the official video doesn't turn up
								beside it. Applies to newly loaded content.
							</p>
						</div>
						<Switch checked={hideVideosOn} onCheckedChange={setHideVideos} />
					</div>
					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">yt-dlp fallback</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Last resort for tracks every YouTube client refuses (restricted/DRM uploads):
								resolve them through a self-updating yt-dlp binary.
								<span class="mt-0.5 block font-mono text-xs text-muted-foreground/80">
									{ytdlp.installed ? 'yt-dlp installed' : 'yt-dlp not installed yet'}
									{ytdlp.last_error ? ` — ${ytdlp.last_error}` : ''}
								</span>
							</p>
							{#if !ytdlp.installed}
								<Button size="sm" variant="outline" class="mt-2" onclick={installYtdlp}>Install now</Button>
							{/if}
						</div>
						<Switch checked={ytdlpOn} onCheckedChange={setYtdlp} />
					</div>
					<div class="py-3">
						<div class="font-medium">Stream clients</div>
						<p class="mt-0.5 mb-2 text-sm text-muted-foreground">
							Advanced — turn a client off to skip it when resolving streams. Overridden by the
							<span class="font-mono text-xs">LIMUSIC_DISABLED_CLIENTS</span> env var.
						</p>
						<div class="flex flex-col gap-2">
							{#each clients as name (name)}
								<div class="flex items-center justify-between">
									<span class="font-mono text-sm">{name}</span>
									<Switch
										checked={!disabled.has(name)}
										onCheckedChange={() => toggleClient(name)}
									/>
								</div>
							{/each}
						</div>
					</div>
					<!-- EQ -->
					<div class="border-t mt-3 pt-3">
						<div class="flex items-center justify-between">
							<div class="font-medium">10-band EQ</div>
							<Button size="sm" variant="ghost" onclick={eqReset}>Reset</Button>
						</div>
						<p class="mt-0.5 mb-3 text-sm text-muted-foreground">31 Hz – 16 kHz peaking, preamp & balance. Uses mpv lavfi equalizer.</p>
						<div class="grid grid-cols-5 gap-3">
							{#each eq.freqs as f, i}
								<div class="flex flex-col items-center gap-1 rounded-lg border bg-card/50 p-2">
									<span class="text-[10px] font-medium text-muted-foreground">{f >= 1000 ? `${f/1000}k` : f} Hz</span>
									<Slider type="single" orientation="vertical" min={-12} max={12} step={0.5} value={eq.bands[i] ?? 0} onValueChange={(v)=>eqSetBand(i, v)} class="h-24" aria-label="{f} Hz" />
									<span class="font-mono text-xs">{(eq.bands[i] ?? 0).toFixed(1)} dB</span>
								</div>
							{/each}
						</div>
						<div class="mt-3 grid grid-cols-3 gap-3">
							<div class="flex flex-col gap-1">
								<span class="text-xs font-medium">Preamp</span>
								<div class="flex items-center gap-2">
									<Slider type="single" min={-12} max={12} step={0.5} value={eq.preamp} onValueChange={(v)=>{eq.preamp=v; api.setPreamp(v);}} class="flex-1" />
									<span class="font-mono text-xs w-12 text-right">{eq.preamp.toFixed(1)} dB</span>
								</div>
							</div>
							<div class="flex flex-col gap-1">
								<span class="text-xs font-medium">Balance</span>
								<div class="flex items-center gap-2">
									<Slider type="single" min={-1} max={1} step={0.1} value={eq.balance} onValueChange={(v)=>{eq.balance=v; api.setBalance(v);}} class="flex-1" />
									<span class="font-mono text-xs w-10 text-right">{eq.balance.toFixed(1)}</span>
								</div>
							</div>
							<div class="flex flex-col gap-1">
								<span class="text-xs font-medium">Output trim</span>
								<div class="flex items-center gap-2">
									<Slider type="single" min={-12} max={12} step={0.5} value={eq.output_gain} onValueChange={(v)=>{eq.output_gain=v; api.setOutputGain(v);}} class="flex-1" />
									<span class="font-mono text-xs w-12 text-right">{eq.output_gain.toFixed(1)} dB</span>
								</div>
							</div>
						</div>
						<div class="mt-3 flex flex-col gap-2">
							<div class="flex items-center justify-between">
								<span class="text-sm">AutoEq</span>
								<Switch checked={eq.auto_eq} onCheckedChange={(on)=>{eq.auto_eq=on; api.setAutoeq(on);}} />
							</div>
							<div class="flex items-center gap-2">
								<Input placeholder="videoId for per-track trim" class="flex-1" value={trackGainId} oninput={(e)=>trackGainId=e.currentTarget.value} />
								<Input type="number" class="w-20" value={String(trackGainVal)} oninput={(e)=>trackGainVal=parseFloat(e.currentTarget.value)||0} />
								<Button size="sm" variant="outline" onclick={()=>{ if(trackGainId) api.setTrackGain(trackGainId, trackGainVal); toast.success('Per-track gain saved'); }}>Save</Button>
							</div>
							<div class="flex items-center justify-between">
								<span class="text-sm">Output device</span>
								<Select.Root type="single" value={"auto"} onValueChange={(v)=>api.setOutputDevice(v)}>
									<Select.Trigger class="w-44"><span>{outputDevices[0] ?? 'auto'}</span></Select.Trigger>
									<Select.Content>
										{#each outputDevices as d}<Select.Item value={d} label={d}>{d}</Select.Item>{/each}
									</Select.Content>
								</Select.Root>
							</div>
						</div>
					</div>
					<!-- Smart Crossfade / Best Mix -->
					<div class="border-t mt-3 pt-3">
						<div class="font-medium">Smart Crossfade & Best Mix</div>
						<p class="mt-0.5 mb-3 text-sm text-muted-foreground">Gapless via mpv gapless-audio; crossfade is a volume ramp hint (1–12s).</p>
						<div class="flex items-center justify-between gap-4 py-2">
							<span class="text-sm">Crossfade {crossfade.secs.toFixed(1)}s</span>
							<Slider type="single" min={0} max={12} step={0.5} value={crossfade.secs} onValueChange={(v)=>setCrossfadeSecs(v)} class="w-40" />
						</div>
						<div class="flex items-center gap-2 py-2">
							<span class="text-sm">Mode</span>
							<div class="flex gap-2">
								<Button size="sm" variant={crossfade.mode==='standard' ? 'default' : 'outline'} onclick={()=>{crossfade.mode='standard'; api.setCrossfade(crossfade.secs,'standard');}}>Standard</Button>
								<Button size="sm" variant={crossfade.mode==='smart' ? 'default' : 'outline'} onclick={()=>{crossfade.mode='smart'; api.setCrossfade(crossfade.secs,'smart');}}>Smart</Button>
							</div>
							<span class="ml-auto text-sm">Best Mix</span>
							<Switch checked={crossfade.best_mix} onCheckedChange={(on)=>{crossfade.best_mix=on; api.setBestMix(on);}} />
						</div>
					</div>
				{:else if tab === 'downloads'}
					<div class="border-b py-3">
						<div class="font-medium">Download location</div>
						<p class="mt-0.5 mb-3 text-sm text-muted-foreground">
							Where offline tracks are saved. Defaults to the app data folder if empty.
						</p>
						<div class="flex items-center gap-2">
							<Input class="flex-1" readonly value={downloadDir} placeholder="App data / downloads" />
							<Button size="sm" variant="outline" onclick={pickDownloadDir}>Browse…</Button>
						</div>
					</div>
					<div class="border-b py-3">
						<div class="font-medium">Default quality</div>
						<p class="mt-0.5 mb-3 text-sm text-muted-foreground">
							Quality used when you download a track for offline listening.
						</p>
						<div class="flex gap-2">
							{#each QUALITIES as q (q.id)}
								<Button
									variant={downloadQuality === q.id ? 'default' : 'outline'}
									size="sm"
									onclick={() => setDownloadQuality(q.id)}
								>{q.label}</Button>
							{/each}
						</div>
					</div>
					<div class="border-b py-3">
						<div class="font-medium">Audio format</div>
						<p class="mt-0.5 mb-3 text-sm text-muted-foreground">
							Container/codec for saved files. M4A is the most compatible.
						</p>
						<div class="flex gap-2">
							{#each DOWNLOAD_FORMATS as f (f.id)}
								<Button
									variant={downloadFormat === f.id ? 'default' : 'outline'}
									size="sm"
									onclick={() => setDownloadFormat(f.id)}
								>{f.label}</Button>
							{/each}
						</div>
					</div>
					<div class="flex items-start justify-between gap-4 border-b py-3">
						<div class="min-w-0">
							<div class="font-medium">Use downloads when available</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								Play the saved file instead of streaming whenever you have one — works offline and
								saves bandwidth.
							</p>
						</div>
						<Switch checked={useOffline} onCheckedChange={setUseOffline} />
					</div>
					<div class="py-3">
						<div class="flex items-center justify-between">
							<div class="font-medium">Downloaded tracks</div>
							<Button size="sm" variant="ghost" disabled={downloads.length === 0} onclick={clearAllDownloads}>
								Clear all
							</Button>
						</div>
						{#if downloads.length === 0}
							<p class="mt-2 text-sm text-muted-foreground">
								Nothing saved yet. Use the ⋮ menu on any track and choose “Download”.
							</p>
						{:else}
							<div class="mt-2 flex flex-col gap-1">
								{#each downloads as d (d.video_id)}
									<div class="flex items-center justify-between gap-3 rounded-md border px-3 py-2">
										<div class="min-w-0">
											<div class="truncate text-sm font-medium">{d.title}</div>
											<div class="truncate text-xs text-muted-foreground">{d.artists}</div>
										</div>
										<div class="flex items-center gap-3 text-xs text-muted-foreground">
											<span class="uppercase">{d.format}</span>
											<span>{fmtSize(d.size_bytes)}</span>
											<Button size="sm" variant="ghost" onclick={() => removeDownload(d.video_id)}>Remove</Button>
										</div>
									</div>
								{/each}
							</div>
						{/if}
					</div>
{:else if tab === 'data'}
					<div class="border-b py-3">
						<div class="font-medium">Proxy</div>
						<p class="mt-0.5 mb-3 text-sm text-muted-foreground">
							HTTP/SOCKS proxy for all YouTube traffic. Takes effect on restart.
						</p>
						<form
							class="flex gap-2"
							onsubmit={(e) => {
								e.preventDefault();
								saveProxy();
							}}
						>
							<Input bind:value={proxyInput} placeholder="http://host:port (blank = none)" />
							<Button type="submit" variant="outline">Save</Button>
						</form>
					</div>
					<div class="py-3">
						<div class="font-medium">Cache</div>
						<p class="mt-0.5 mb-3 text-sm text-muted-foreground">
							Clear cached stream URLs and downloaded audio bytes.
						</p>
						<Button variant="destructive" size="sm" onclick={doClearCaches} disabled={clearing}>
							{clearing ? 'Clearing…' : 'Clear caches'}
						</Button>
					</div>
				{:else if tab === 'about'}
					<div class="border-b py-3">
						<div class="font-heading text-lg font-bold">Limusic</div>
						<p class="mt-1 text-sm text-muted-foreground">
							A cross-platform desktop YouTube Music client — ad-free playback straight from
							YouTube's private API, with your real library and OS media keys.
						</p>
						{#if version}<p class="mt-2 text-sm text-muted-foreground">Version {version}</p>{/if}
					</div>
					<div class="flex items-center justify-between gap-4 py-3">
						<div class="min-w-0">
							<div class="font-medium">Updates</div>
							<p class="mt-0.5 text-sm text-muted-foreground">
								{#if updateState.available && !updateState.canInstall}
									Version {updateState.available.version} is available. This build was installed by a
									package manager, so update it the same way.
								{:else if updateState.available}
									Version {updateState.available.version} is available.
								{:else}
									Check GitHub for a newer release.
								{/if}
							</p>
						</div>
						{#if updateState.available && !updateState.canInstall}
							<Button size="sm" onclick={openDownloadPage}>Download</Button>
						{:else if updateState.available}
							<Button size="sm" onclick={installUpdate} disabled={updateState.installing}>
								{updateState.installing ? 'Updating…' : 'Update now'}
							</Button>
						{:else}
							<Button
								variant="outline"
								size="sm"
								onclick={checkUpdates}
								disabled={updateState.checking}
							>
								{updateState.checking ? 'Checking…' : 'Check for updates'}
							</Button>
						{/if}
					</div>
					{#if updateResult && !updateState.available}
						<Alert variant={updateResult.error ? 'destructive' : 'default'}>
							<AlertDescription>{updateResult.message}</AlertDescription>
						</Alert>
					{/if}
				{/if}
			</div>
		</div>
	</Dialog.Content>
</Dialog.Root>
