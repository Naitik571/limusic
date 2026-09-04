<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import { open } from '@tauri-apps/plugin-dialog';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Cancel01Icon,
		Settings02Icon,
		PaintBoardIcon,
		PlayCircleIcon,
		Download04Icon,
		Database02Icon,
		InformationCircleIcon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Switch } from '$lib/components/ui/switch';
	import { Slider } from '$lib/components/ui/slider';
	import { Alert, AlertDescription } from '$lib/components/ui/alert';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as api from '$lib/api';
	import { ui, toast, markNotDownloaded, downloadedIds, crossfade, loadCrossfade, setCrossfadeSecs, setCrossfadeMode, setBestMix, sleepTimer, setSleepTimer } from '$lib/player.svelte';
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
		LYRIC_FONTS,
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
	import { setLocale, currentLocale, LOCALES, type LocaleId } from '$lib/i18n.svelte';

	type TabId = 'general' | 'themes' | 'playback' | 'downloads' | 'data' | 'about';
	const TABS: { id: TabId; label: string; hint: string; icon: typeof Settings02Icon }[] = [
		{ id: 'general', label: 'General', hint: 'History, integrations and how the app starts.', icon: Settings02Icon },
		{ id: 'themes', label: 'Appearance', hint: 'Colors, fonts, layouts and the player view.', icon: PaintBoardIcon },
		{ id: 'playback', label: 'Playback', hint: 'Quality, transitions and streams.', icon: PlayCircleIcon },
		{ id: 'downloads', label: 'Downloads', hint: 'Offline files: location, quality and cleanup.', icon: Download04Icon },
		{ id: 'data', label: 'Data & storage', hint: 'Network and cached files.', icon: Database02Icon },
		{ id: 'about', label: 'About', hint: 'Version and updates.', icon: InformationCircleIcon }
	];

	// Shared shapes for the settings rows. Kept as strings so the markup below stays readable and
	// every group looks identical without a wrapper component per row.
	const GROUP = 'mb-7 last:mb-1';
	const LABEL =
		'mb-2 px-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground';
	const CARD = 'divide-y divide-border/60 overflow-hidden rounded-xl border bg-card';

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
	const currentTab = $derived(TABS.find((t) => t.id === tab) ?? TABS[0]);

	// --- Settings search -----------------------------------------------------------------------------------
	// One text field filters the current tab live and, when nothing matches here, points at the
	// tabs that do. Two layers: `rowMatch` hides individual rows (inside the shared `row`
	// snippet), `groupVisible` collapses a whole section once none of its rows survive. Both read
	// the same static keyword index below — one entry per row's title+desc, so the chips and the
	// collapsing always agree with what's on screen.
	let search = $state('');
	const qnorm = $derived(search.trim().toLowerCase());

	type SearchEntry = { tab: TabId; group: string; text: string };
	const SEARCH_INDEX: SearchEntry[] = [
		// General
		{ tab: 'general', group: 'gen-activity', text: 'Watch history Register plays in your YouTube Music history. Needs sign-in.' },
		{ tab: 'general', group: 'gen-activity', text: "Discord rich presence Show what you're listening to on your Discord profile. Needs the Discord desktop app running." },
		{ tab: 'general', group: 'gen-system', text: 'Close to tray Closing the window keeps music playing in the background. Restore or quit from the tray icon.' },
		{ tab: 'general', group: 'gen-system', text: 'Start on login Launch Limusic automatically when you log in.' },
		{ tab: 'general', group: 'gen-system', text: 'Interface language Switch the app between the bundled languages (English, Turkish).' },
		{ tab: 'general', group: 'gen-lyrics', text: 'Apple Music lyrics Paste two values from a logged-in music.apple.com session to unlock word-level lyrics. Media user token and developer bearer token.' },
		{ tab: 'general', group: 'gen-remote', text: 'Remote LAN Control Control playback from your phone on the same Wi-Fi. Scan the QR or open the URL. Pairing token.' },
		// Appearance
		{ tab: 'themes', group: 'thm-theme', text: 'Preset Accent colors tint the default look; palettes swap every color.' },
		{ tab: 'themes', group: 'thm-theme', text: 'Accent color Buttons, highlights and the progress bar. Applies over any preset.' },
		{ tab: 'themes', group: 'thm-theme', text: 'Background tint Shades the greys: surfaces, borders and secondary text.' },
		{ tab: 'themes', group: 'thm-theme', text: 'Roundness Corner radius of cards, buttons and artwork.' },
		{ tab: 'themes', group: 'thm-theme', text: 'Reset customization Drop the color, roundness and font overrides. Keeps the preset.' },
		{ tab: 'themes', group: 'thm-layout', text: 'Window layout Orchard window arrangement — Grove, Canopy and more.' },
		{ tab: 'themes', group: 'thm-typography', text: 'Interface font Everything except headings.' },
		{ tab: 'themes', group: 'thm-typography', text: 'Heading font Page and section titles.' },
		{ tab: 'themes', group: 'thm-typography', text: 'Font files Load a .ttf, .otf or .woff from anywhere on this computer.' },
		{ tab: 'themes', group: 'thm-typography', text: 'Lyrics font Choose the font used only in the lyrics view.' },
		{ tab: 'themes', group: 'thm-player', text: 'Queue and lyrics in the player view Tabs and switching buttons in the player view.' },
		{ tab: 'themes', group: 'thm-player', text: "Artwork background Tint the player view with the playing track's cover, blurred." },
		{ tab: 'themes', group: 'thm-player', text: "Adapt colors to artwork Recolor the app from the playing track's cover: accent, surfaces and borders." },
		{ tab: 'themes', group: 'thm-backdrops', text: 'Backdrop Off Subtle Auto artwork atmosphere behind the app.' },
		{ tab: 'themes', group: 'thm-backdrops', text: 'Spotify Canvas Show looping Canvas video in Now Playing when available.' },
		{ tab: 'themes', group: 'thm-packs', text: 'Get packs Per-artist ZIPs indexed every 15min. Injects style.css on the artist page.' },
		{ tab: 'themes', group: 'thm-packs', text: 'Installed packs Packs currently installed.' },
		// Playback
		{ tab: 'playback', group: 'pb-audio', text: 'Audio quality Preferred stream quality when resolving a track.' },
		{ tab: 'playback', group: 'pb-audio', text: 'Autoplay Keep the music going with similar songs when your queue ends.' },
		{ tab: 'playback', group: 'pb-audio', text: "Prevent duplicate tracks in queue Adding a track that's already in the queue moves it from its old position." },
		{ tab: 'playback', group: 'pb-audio', text: 'Keep shuffle across queue When shuffle is on, opening an album/playlist/radio appends to the queue instead of resetting playback.' },
		{ tab: 'playback', group: 'pb-audio', text: 'Sleep timer Stop playback after a while. Off End of song minutes.' },
		{ tab: 'playback', group: 'pb-transitions', text: 'Smart Crossfade Gapless via mpv gapless-audio; crossfade is a volume ramp hint.' },
		{ tab: 'playback', group: 'pb-transitions', text: 'Crossfade mode Standard Smart.' },
		{ tab: 'playback', group: 'pb-transitions', text: 'Best Mix' },
		{ tab: 'playback', group: 'pb-video', text: "Hide music videos Keep only the audio version of a track, so the official video doesn't turn up beside it." },
		{ tab: 'playback', group: 'pb-video', text: 'yt-dlp fallback Last resort for tracks every YouTube client refuses. Resolve them through a self-updating yt-dlp binary.' },
		{ tab: 'playback', group: 'pb-video', text: 'Stream clients Turn a client off to skip it when resolving streams.' },
		// Downloads
		{ tab: 'downloads', group: 'dl-location', text: 'Download location Where offline tracks are saved. Defaults to the app data folder if empty.' },
		{ tab: 'downloads', group: 'dl-quality', text: 'Default quality Quality used when you download a track for offline listening.' },
		{ tab: 'downloads', group: 'dl-quality', text: 'Audio format Container/codec for saved files. M4A is the most compatible.' },
		{ tab: 'downloads', group: 'dl-quality', text: 'Use downloads when available Play the saved file instead of streaming whenever you have one.' },
		{ tab: 'downloads', group: 'dl-auto', text: 'Keep new music offline automatically New liked songs are fetched in the background — no manual downloads.' },
		{ tab: 'downloads', group: 'dl-saved', text: 'Downloaded tracks Saved tracks and their size.' },
		// Data & storage
		{ tab: 'data', group: 'dt-network', text: 'Proxy HTTP/SOCKS proxy for all YouTube traffic. Takes effect on restart.' },
		{ tab: 'data', group: 'dt-storage', text: 'Cache Clear cached stream URLs and downloaded audio bytes.' },
		// About
		{ tab: 'about', group: 'ab-hero', text: 'Limusic cross-platform desktop YouTube Music client. Ad-free playback straight from YouTube private API, real library and OS media keys.' },
		{ tab: 'about', group: 'ab-updates', text: 'Updates Check GitHub for a newer release. Version available. Update now. Download.' }
	];

	function rowMatch(title: string, desc?: string) {
		if (!qnorm) return true;
		return title.toLowerCase().includes(qnorm) || (desc ?? '').toLowerCase().includes(qnorm);
	}

	function groupVisible(group: string) {
		if (!qnorm) return true;
		return SEARCH_INDEX.some((e) => e.group === group && e.text.toLowerCase().includes(qnorm));
	}

	const tabHasMatches = $derived(
		!qnorm || SEARCH_INDEX.some((e) => e.tab === tab && e.text.toLowerCase().includes(qnorm))
	);
	const otherTabsWithQuery = $derived(
		qnorm
			? TABS.filter(
					(t) =>
						t.id !== tab &&
						SEARCH_INDEX.some((e) => e.tab === t.id && e.text.toLowerCase().includes(qnorm))
				)
			: []
	);

	function switchTab(t: TabId) {
		tab = t; // query stays applied — that's the point of the chips
	}

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
	/** Real QR (SVG markup) for the LAN URL, generated by Rust. Needs a light background to scan. */
	let qrSvg = $state('');
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
			loadCrossfade();
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
	// One Backdrop control replaces three rows (artwork background, ambient mode, ambient
	// intensity): off = plain surfaces, subtle = player tint only, auto = full blurred wash.
	type BackdropMode = 'off' | 'subtle' | 'auto';
	const BACKDROP_MODES = [
		{ id: 'off', label: 'Off' },
		{ id: 'subtle', label: 'Subtle' },
		{ id: 'auto', label: 'Auto' }
	];
	const backdrop = $derived<BackdropMode>(
		appearance.ambientMode ? 'auto' : appearance.artworkBackground ? 'subtle' : 'off'
	);
	function setBackdrop(mode: BackdropMode) {
		if (mode === 'off') setAppearance({ artworkBackground: false, ambientMode: false });
		else if (mode === 'subtle') setAppearance({ artworkBackground: true, ambientMode: false });
		else setAppearance({ artworkBackground: true, ambientMode: true, ambientIntensity: 'balanced' });
	}
	const historyOn = $derived(settings.enable_history !== 'false');
	const autoplayOn = $derived(settings.autoplay !== 'false');
	const hideVideosOn = $derived(settings.hide_videos === 'true');
	const preventDuplicatesOn = $derived(settings.prevent_duplicates === 'true');
	const discordOn = $derived(settings.discord_rpc === 'true');
	const trayOn = $derived(settings.close_to_tray !== 'false');
	const autostartOn = $derived(settings.autostart === 'true');
	const ytdlpOn = $derived(settings.ytdlp_enabled !== 'false');
	const stickyShuffleOn = $derived(settings.sticky_shuffle === 'true');
	// Sleep timer badge: live countdown while a minutes timer runs.
	const sleepBadge = $derived(
		sleepTimer.mode === 'off'
			? undefined
			: sleepTimer.mode === 'end_of_song'
				? 'End of song'
				: `${Math.floor(sleepTimer.remaining / 60)}:${String(sleepTimer.remaining % 60).padStart(2, '0')} left`
	);
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

	const AUTO_OFFLINE_MODES: { id: string; label: string }[] = [
		{ id: 'off', label: 'Off' },
		{ id: 'liked', label: 'Liked Music' },
		{ id: 'liked_playlists', label: 'Likes + playlists' }
	];
	const autoOffline = $derived(settings.auto_offline ?? 'off');

	async function setAutoOffline(mode: string) {
		settings.auto_offline = mode;
		await api.setSetting('auto_offline', mode);
		if (mode !== 'off') {
			toast.info('Syncing your Liked Music…');
			api.autoOfflineSync().catch(() => {});
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

	async function setStickyShuffle(on: boolean) {
		settings.sticky_shuffle = on ? 'true' : 'false';
		await api.setSetting('sticky_shuffle', settings.sticky_shuffle);
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

	// --- Remote QR helpers: the QR itself is rendered by Rust (qrcode crate → SVG) so the code
	// actually scans; the UI just puts it on a light, padded surface.
	async function refreshRemote() {
		try {
			lanUrl = await api.getLanUrl();
			remoteToken = await api.getRemoteToken();
			qrSvg = await api.getRemoteQr();
		} catch {}
	}
	async function regenerateToken() {
		try {
			remoteToken = await api.pairRemote('__regenerate__').then(() => api.getRemoteToken());
			lanUrl = await api.getLanUrl();
			qrSvg = await api.getRemoteQr();
			toast.success('Refreshed');
		} catch {
			await refreshRemote();
			toast.success('Refreshed');
		}
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

<!-- One row shape for the whole modal: label and description on the left, the control on the right,
     and an optional block underneath for the things that expand (color picker, font input, lists,
     the EQ sliders, the QR canvas, the layouts preview). -->
{#snippet row(o: {
	title: string;
	desc?: string;
	badge?: string;
	control?: Snippet;
	below?: Snippet;
	tall?: boolean;
})}
	<!-- Settings search: a non-empty query that matches neither the title nor the description
	     skips the row entirely; sections collapse via groupVisible() when nothing survives. -->
	{#if rowMatch(o.title, o.desc)}
		<div class="px-4 py-3.5">
			<div class="flex {o.tall ? 'items-start' : 'items-center'} justify-between gap-6">
				<div class="min-w-0">
					<div class="flex items-center gap-2">
						<span class="text-sm font-medium">{o.title}</span>
						{#if o.badge}
							<span
								class="rounded-full bg-primary/12 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-primary"
							>
								{o.badge}
							</span>
						{/if}
					</div>
					{#if o.desc}
						<p class="mt-1 max-w-prose text-xs leading-relaxed text-muted-foreground">{o.desc}</p>
					{/if}
				</div>
				{#if o.control}
					<div class="shrink-0">{@render o.control()}</div>
				{/if}
			</div>
			{#if o.below}
				<div class="mt-3">{@render o.below()}</div>
			{/if}
		</div>
	{/if}
{/snippet}

<Dialog.Root bind:open={ui.settingsOpen}>
	<Dialog.Content class="gap-0 overflow-hidden p-0 sm:max-w-3xl">
		<Dialog.Description class="sr-only">Application settings</Dialog.Description>

		<div class="flex h-[min(38rem,80vh)]">
			<!-- Tab rail -->
			<nav class="flex w-52 shrink-0 flex-col border-r bg-muted/40 p-3">
				<Dialog.Title class="px-3 pt-1 pb-4 font-heading text-base font-semibold">
					Settings
				</Dialog.Title>
				<div class="flex flex-col gap-0.5">
					{#each TABS as t (t.id)}
						<button
							onclick={() => (tab = t.id)}
							aria-current={tab === t.id}
							class="flex w-full cursor-pointer items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm font-medium transition-colors {tab === t.id ? 'bg-background text-foreground shadow-sm ring-1 ring-border/70' : 'text-muted-foreground hover:bg-foreground/5 hover:text-foreground'}"
						>
							<HugeiconsIcon
								icon={t.icon}
								size={17}
								strokeWidth={2}
								class={tab === t.id ? 'text-primary' : ''}
							/>
							<span class="truncate">{t.label}</span>
						</button>
					{/each}
				</div>
				{#if version}
					<span class="mt-auto px-3 pb-1 text-[11px] text-muted-foreground">v{version}</span>
				{/if}
			</nav>

			<!-- Content pane. min-w-0: a flex child's min-width is auto, so without it one wide row
			     (a long font name, a long path) widens the pane and pushes every tab off the modal.
			     min-h-0: same on the vertical axis — without it the pane grows with long tabs
			     (Downloads) instead of letting the scroller below scroll, and the overflow is
			     clipped by the dialog edge. -->
			<div class="flex min-h-0 min-w-0 flex-1 flex-col">
				<!-- h-14 also keeps the dialog's close button clear of the first row. -->
				<header class="flex h-14 shrink-0 flex-col justify-center border-b px-6 pr-14">
					<h2 class="text-sm font-semibold">{currentTab.label}</h2>
					<p class="truncate text-xs text-muted-foreground">{currentTab.hint}</p>
				</header>

				<div class="min-w-0 flex-1 overflow-x-hidden overflow-y-auto px-6 py-5">
					<!-- Settings search: sits in flow above the content (sticky was overlapping the tab
					     header on short tabs), still the first thing in the pane. -->
					<div class="mb-4 flex items-center gap-2">
						<Input
							bind:value={search}
							placeholder="Search settings…"
							aria-label="Search settings"
							class="h-8 min-w-0 flex-1"
						/>
						{#if search}
							<Button
								variant="ghost"
								size="icon-sm"
								aria-label="Clear settings search"
								onclick={() => (search = '')}
							>
								<HugeiconsIcon icon={Cancel01Icon} size={14} />
							</Button>
						{/if}
					</div>

					{#if qnorm && !tabHasMatches}
						<div
							class="mb-4 flex flex-wrap items-center gap-1.5 rounded-lg border border-dashed bg-muted/40 px-3 py-2"
						>
							<span class="text-xs text-muted-foreground">
								No matches in {currentTab.label} — found in:
							</span>
							{#each otherTabsWithQuery as t (t.id)}
								<Button
									size="sm"
									variant="outline"
									class="h-6 rounded-full px-2.5 text-xs"
									onclick={() => switchTab(t.id)}
								>
									{t.label}
								</Button>
							{/each}
						</div>
					{/if}

					{#if !loaded}
						<p class="text-sm text-muted-foreground">Loading…</p>
					{:else if tab === 'general'}
						<section class="{GROUP} {groupVisible('gen-activity') ? '' : 'hidden'}">
							<h3 class={LABEL}>Activity</h3>
							<div class={CARD}>
								{@render row({
									title: 'Watch history',
									desc: 'Register plays in your YouTube Music history. Needs sign-in.',
									control: historySwitch
								})}
								{@render row({
									title: 'Discord rich presence',
									desc: "Show what you're listening to on your Discord profile. Needs the Discord desktop app running — no login here.",
									control: discordSwitch
								})}
							</div>
						</section>
						<section class="{GROUP} {groupVisible('gen-system') ? '' : 'hidden'}">
							<h3 class={LABEL}>System</h3>
							<div class={CARD}>
								{@render row({
									title: 'Close to tray',
									desc: 'Closing the window keeps music playing in the background. Restore or quit from the tray icon.',
									control: traySwitch
								})}
								{@render row({
								title: 'Start on login',
								desc: 'Launch Limusic automatically when you log in.',
								control: autostartSwitch
								})}
								</div>
								</section>
								<section class="{GROUP} {groupVisible('gen-system') ? '' : 'hidden'}">
								<h3 class={LABEL}>Language</h3>
								<div class={CARD}>
								{@render row({
									title: 'Interface language',
									desc: 'The app language. English and Turkish are bundled; more can be added as locale files.',
									control: languageSelect
								})}
								</div>
								</section>
								<section class="{GROUP} {groupVisible('gen-lyrics') ? '' : 'hidden'}">
							<h3 class={LABEL}>Lyrics</h3>
							<div class={CARD}>
								{@render row({
									title: 'Apple Music lyrics',
									badge: 'Optional',
									desc: "Paste two values from a logged-in music.apple.com session to unlock Apple's word-level lyrics: the media user token and the developer bearer token (both are in the site's request headers — any devtools network tab shows them). Stored only on this machine. Leave empty to keep it off.",
									below: appleForm
								})}
							</div>
						</section>
						<section class="{GROUP} {groupVisible('gen-remote') ? '' : 'hidden'}">
							<h3 class={LABEL}>Remote</h3>
							<div class={CARD}>
								{@render row({
									title: 'Remote LAN Control',
									desc: 'Control playback from your phone on the same Wi-Fi. Scan the QR or open the URL. Pairing token (128-bit base64url) stored in the DB; HTTP listens on 0.0.0.0:32145. Approve/deny handled via token match.',
									below: remotePanel
								})}
							</div>
						</section>
					{:else if tab === 'themes'}
						<section class="{GROUP} {groupVisible('thm-theme') ? '' : 'hidden'}">
							<h3 class={LABEL}>Theme</h3>
							<div class={CARD}>
								{@render row({
									title: 'Preset',
									desc: 'Accent colors tint the default look; palettes swap every color.',
									control: presetSelect
								})}
								{@render row({
									title: 'Accent color',
									desc: 'Buttons, highlights and the progress bar. Applies over any preset.',
									control: accentSwatch,
									below: pickerOpen ? accentPicker : undefined
								})}
								{@render row({
									title: 'Background tint',
									desc:
										currentTheme.kind === 'palette'
											? `Only shades the default palette — ${currentTheme.label} brings its own colors.`
											: 'Shades the greys: surfaces, borders and secondary text.',
									control: tintSlider
								})}
							{@render row({
								title: 'Roundness',
								desc: 'Corner radius of cards, buttons and artwork.',
								control: radiusSlider
							})}
						{@render row({
								title: 'Reset customization',
								desc: 'Drop the color, roundness and font overrides. Keeps the preset.',
								control: resetButton
							})}
							</div>
						</section>

						<section class="{GROUP} {groupVisible('thm-layout') ? '' : 'hidden'}">
							<h3 class={LABEL}>Layout</h3>
							<div class={CARD}>
								{@render row({
									title: 'Window layout',
									desc: 'Orchard window arrangement — Grove, Canopy and more.',
									control: layoutSelect,
									below: layoutPreview
								})}
							</div>
						</section>

						<section class="{GROUP} {groupVisible('thm-typography') ? '' : 'hidden'}">
							<h3 class={LABEL}>Typography</h3>
							<div class={CARD}>
								{#each FONT_ROWS as fr (fr.key)}
									<!-- Zero-arg wrappers: a snippet passed as a value can't carry arguments. -->
									{#snippet pick()}{@render fontSelect(fr.key, fr.label)}{/snippet}
									{#snippet type()}{@render fontInput(fr.key, fr.label)}{/snippet}
									{@render row({
										title: fr.label,
										desc: fr.hint,
										control: pick,
										below: isCustomFont[fr.key] ? type : undefined
									})}
								{/each}
								{@render row({
									title: 'Font files',
									desc: 'Load a .ttf, .otf or .woff from anywhere on this computer. It joins both dropdowns above.',
									control: addFontButton,
									below: custom.fontFiles.length ? fontFileList : undefined
								})}
								{@render row({
									title: 'Lyrics font',
									desc: 'Choose the font used only in the lyrics view.',
									control: lyricsFontSelect
								})}
								</div>
								</section>

						<section class="{GROUP} {groupVisible('thm-player') ? '' : 'hidden'}">
							<h3 class={LABEL}>Player view</h3>
							<div class={CARD}>
								{@render row({
									title: 'Queue and lyrics in the player view',
									desc: "On, the player view carries them as tabs and the bar's two buttons switch between them. Off, those buttons only ever open the side panels, which stay open over the player view so you can see both at once.",
									control: tabbedSwitch,
									tall: true
								})}
								{@render row({
									title: 'Adapt colors to artwork',
									badge: 'Experimental',
									desc: "Recolor the app from the playing track's cover: accent, surfaces and borders, fading between tracks. Off keeps the selected theme's own colors.",
									control: artworkAccentSwitch,
									tall: true
								})}
							</div>
						</section>

						<section class="{GROUP} {groupVisible('thm-backdrops') ? '' : 'hidden'}">
							<h3 class={LABEL}>Backdrops</h3>
							<div class={CARD}>
								{@render row({
									title: 'Backdrop',
									desc: 'Artwork atmosphere behind the app. Off is plain surfaces, Subtle tints the player view, Auto adds the full blurred wash.',
									control: backdropPicker
								})}
								{@render row({
									title: 'Spotify Canvas',
									badge: 'Auto',
									desc: 'Show looping Canvas video in Now Playing when available (#8). Fetches via https://api.simpmusic.org/canvas (or Spotify API stub) with palette gradient fallback, muted autoplay loop.'
								})}
							</div>
						</section>

						<section class="{GROUP} {groupVisible('thm-packs') ? '' : 'hidden'}">
							<h3 class={LABEL}>Artist packs</h3>
							<div class={CARD}>
								{@render row({
									title: 'Get packs',
									desc: 'Per-artist ZIPs (artist.json + style.css) indexed from R2 artist-packs.sfg545.dev/v1/index.json every 15min. Injects style.css via data URI on the artist page; stored under app_data/artist_packs/<id>/.',
									control: packRefreshButton,
									below: packInstall
								})}
								{@render row({ title: 'Installed packs', below: packList })}
							</div>
						</section>
					{:else if tab === 'playback'}
						<section class="{GROUP} {groupVisible('pb-audio') ? '' : 'hidden'}">
							<h3 class={LABEL}>Audio</h3>
							<div class={CARD}>
								{@render row({
									title: 'Audio quality',
									desc: 'Preferred stream quality when resolving a track.',
									control: qualityPicker
								})}
								{@render row({
									title: 'Autoplay',
									desc: 'Keep the music going with similar songs when your queue ends.',
									control: autoplaySwitch
								})}
								{@render row({
									title: 'Prevent duplicate tracks in queue',
									desc: "Adding a track that's already in the queue moves it from its old position instead of adding a second copy.",
									control: dupSwitch,
									tall: true
								})}
							{@render row({
								title: 'Keep shuffle across queue',
								desc: 'When shuffle is on, opening an album/playlist/radio appends to the queue instead of resetting playback.',
								control: stickyShuffleSwitch
							})}
							{@render row({
								title: 'Sleep timer',
								desc: 'Stop playback after a while. Enforced by the backend, so it keeps counting with the window closed.',
								badge: sleepTimer.mode === 'off' ? undefined : sleepBadge,
								below: sleepTimerPresets
							})}
							</div>
						</section>

						<section class="{GROUP} {groupVisible('pb-transitions') ? '' : 'hidden'}">
							<h3 class={LABEL}>Transitions</h3>
							<div class={CARD}>
								{@render row({
									title: 'Smart Crossfade',
									desc: 'Gapless via mpv gapless-audio; crossfade is a volume ramp hint (1–12s).',
									control: crossfadeSlider
								})}
								{@render row({
									title: 'Crossfade mode',
									control: crossfadeMode
								})}
								{@render row({
									title: 'Best Mix',
									control: bestMixSwitch
								})}
							</div>
						</section>

						<section class="{GROUP} {groupVisible('pb-video') ? '' : 'hidden'}">
							<h3 class={LABEL}>Video & advanced</h3>
							<div class={CARD}>
								{@render row({
									title: 'Hide music videos',
									desc: "Keep only the audio version of a track, so the official video doesn't turn up beside it. Applies to newly loaded content.",
									control: hideVideoSwitch,
									tall: true
								})}
								{@render row({
									title: 'yt-dlp fallback',
									desc: `Last resort for tracks every YouTube client refuses (restricted/DRM uploads): resolve them through a self-updating yt-dlp binary. ${ytdlp.installed ? 'yt-dlp installed' : 'yt-dlp not installed yet'}${ytdlp.last_error ? ` — ${ytdlp.last_error}` : ''}`,
									control: ytdlpSwitch,
									tall: true,
									below: ytdlp.installed ? undefined : ytdlpInstall
								})}
								{@render row({
									title: 'Stream clients',
									below: clientList
								})}
							</div>
						</section>
					{:else if tab === 'downloads'}
						<section class="{GROUP} {groupVisible('dl-location') ? '' : 'hidden'}">
							<h3 class={LABEL}>Location</h3>
							<div class={CARD}>
								{@render row({
									title: 'Download location',
									desc: 'Where offline tracks are saved. Defaults to the app data folder if empty.',
									below: downloadDirForm
								})}
							</div>
						</section>
						<section class="{GROUP} {groupVisible('dl-quality') ? '' : 'hidden'}">
							<h3 class={LABEL}>Quality & format</h3>
							<div class={CARD}>
								{@render row({
									title: 'Default quality',
									desc: 'Quality used when you download a track for offline listening.',
									control: downloadQualityPicker
								})}
								{@render row({
									title: 'Audio format',
									desc: 'Container/codec for saved files. M4A is the most compatible.',
									control: downloadFormatPicker
								})}
								{@render row({
									title: 'Use downloads when available',
									desc: 'Play the saved file instead of streaming whenever you have one — works offline and saves bandwidth.',
									control: offlineSwitch,
									tall: true
								})}
							</div>
						</section>
						<section class="{GROUP} {groupVisible('dl-auto') ? '' : 'hidden'}">
							<h3 class={LABEL}>Auto-offline</h3>
							<div class={CARD}>
								{@render row({
									title: 'Keep new music offline automatically',
									desc: 'New liked songs (and playlist adds, in the wider mode) are fetched in the background — no manual downloads. Turning this on also syncs everything already in your Liked Music right away; the walk skips what\'s on disk, so re-runs cost only what\'s missing.',
									control: autoOfflinePicker,
									tall: true
								})}
							</div>
						</section>
						<section class="{GROUP} {groupVisible('dl-saved') ? '' : 'hidden'}">
							<h3 class={LABEL}>Saved tracks</h3>
							<div class={CARD}>
								{@render row({
									title: 'Downloaded tracks',
									control: clearAllButton,
									below: downloadsList
								})}
							</div>
						</section>
					{:else if tab === 'data'}
						<section class="{GROUP} {groupVisible('dt-network') ? '' : 'hidden'}">
							<h3 class={LABEL}>Network</h3>
							<div class={CARD}>
								{@render row({
									title: 'Proxy',
									desc: 'HTTP/SOCKS proxy for all YouTube traffic. Takes effect on restart.',
									below: proxyForm
								})}
							</div>
						</section>
						<section class="{GROUP} {groupVisible('dt-storage') ? '' : 'hidden'}">
							<h3 class={LABEL}>Storage</h3>
							<div class={CARD}>
								{@render row({
									title: 'Cache',
									desc: 'Clear cached stream URLs and downloaded audio bytes.',
									control: clearButton
								})}
							</div>
						</section>
					{:else if tab === 'about'}
						<div
							class="mb-7 rounded-xl border bg-gradient-to-br from-primary/8 to-transparent px-4 py-4 {groupVisible('ab-hero') ? '' : 'hidden'}"
						>
							<div class="flex items-center gap-2">
								<span class="font-heading text-lg font-bold">Limusic</span>
								{#if version}
									<span
										class="rounded-full bg-primary/12 px-2 py-0.5 text-[11px] font-semibold text-primary"
									>
										v{version}
									</span>
								{/if}
							</div>
							<p class="mt-1.5 max-w-prose text-xs leading-relaxed text-muted-foreground">
								A cross-platform desktop YouTube Music client. Ad-free playback straight from
								YouTube's private API, with your real library and OS media keys.
							</p>
						</div>

						<section class="{GROUP} {groupVisible('ab-updates') ? '' : 'hidden'}">
							<h3 class={LABEL}>Updates</h3>
							<div class={CARD}>
								{@render row({
									title: 'Updates',
									desc: updateState.available && !updateState.canInstall
										? `Version ${updateState.available.version} is available. This build was installed by a package manager, so update it the same way.`
										: updateState.available
											? `Version ${updateState.available.version} is available.`
											: 'Check GitHub for a newer release.',
									control: updateButton,
									below: updateResult && !updateState.available ? updateAlert : undefined
								})}
							</div>
						</section>
					{/if}
				</div>
			</div>
		</div>
	</Dialog.Content>
</Dialog.Root>

<!-- Controls. Split out so the rows above read as a list of settings rather than a wall of markup. -->
{#snippet historySwitch()}<Switch checked={historyOn} onCheckedChange={setHistory} />{/snippet}
{#snippet discordSwitch()}<Switch checked={discordOn} onCheckedChange={setDiscord} />{/snippet}
{#snippet traySwitch()}<Switch checked={trayOn} onCheckedChange={setTray} />{/snippet}
{#snippet autostartSwitch()}<Switch checked={autostartOn} onCheckedChange={setAutostart} />{/snippet}
{#snippet languageSelect()}
	<Select.Root
		type="single"
		value={currentLocale.id}
		onValueChange={(v: string) => v && setLocale(v as LocaleId)}
	>
		<Select.Trigger class="w-44 shrink-0" aria-label="Language">
			{LOCALES.find((l) => l.id === currentLocale.id)?.label ?? 'English'}
		</Select.Trigger>
		<Select.Content>
			{#each LOCALES as l (l.id)}
				<Select.Item value={l.id} label={l.label}>
					{l.label}
				</Select.Item>
			{/each}
		</Select.Content>
	</Select.Root>
{/snippet}
{#snippet autoplaySwitch()}<Switch checked={autoplayOn} onCheckedChange={setAutoplay} />{/snippet}
{#snippet dupSwitch()}<Switch
	checked={preventDuplicatesOn}
	onCheckedChange={setPreventDuplicates}
/>{/snippet}
{#snippet stickyShuffleSwitch()}<Switch
	checked={stickyShuffleOn}
	onCheckedChange={setStickyShuffle}
/>{/snippet}
{#snippet hideVideoSwitch()}<Switch checked={hideVideosOn} onCheckedChange={setHideVideos} />{/snippet}
{#snippet ytdlpSwitch()}<Switch checked={ytdlpOn} onCheckedChange={setYtdlp} />{/snippet}
{#snippet offlineSwitch()}<Switch checked={useOffline} onCheckedChange={setUseOffline} />{/snippet}
{#snippet tabbedSwitch()}<Switch
		checked={appearance.tabbedPlayer}
		onCheckedChange={(on) => setAppearance({ tabbedPlayer: on })}
	/>{/snippet}
{#snippet artworkAccentSwitch()}<Switch
		checked={appearance.artworkAccent}
		onCheckedChange={(on) => setAppearance({ artworkAccent: on })}
	/>{/snippet}
{#snippet bestMixSwitch()}<Switch
		checked={crossfade.best_mix}
		onCheckedChange={(on) => {
			crossfade.best_mix = on;
			api.setBestMix(on);
		}}
	/>{/snippet}

{#snippet appleForm()}
	<div class="grid gap-2">
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
{/snippet}

{#snippet remotePanel()}
	<div class="flex gap-4">
		<!-- QR needs a light surface to scan against the dark theme -->
		<div
			class="flex h-48 w-48 shrink-0 items-center justify-center overflow-hidden rounded-lg border bg-white p-3 shadow-sm"
		>
			{#if qrSvg}
				<!-- eslint-disable-next-line svelte/no-at-html-tags — SVG generated by our own Rust command -->
				<div class="h-full w-full [&>svg]:h-full [&>svg]:w-full">{@html qrSvg}</div>
			{:else}
				<span class="text-xs text-neutral-400">Generating…</span>
			{/if}
		</div>
		<div class="min-w-0 flex-1 space-y-2">
			<div class="rounded-lg bg-muted/60 px-3 py-2 font-mono text-xs break-all">
				{lanUrl || 'Loading…'}
			</div>
			<div class="text-xs text-muted-foreground">
				Token: <span class="font-mono">{remoteToken ? remoteToken.slice(0, 8) + '…' : '—'}</span>
			</div>
			<div class="flex gap-2">
				<Button size="sm" variant="outline" onclick={refreshRemote}>Refresh</Button>
				<Button size="sm" variant="ghost" onclick={regenerateToken}>Regenerate</Button>
			</div>
		</div>
	</div>
{/snippet}

{#snippet presetSelect()}
	<Select.Root type="single" value={theme.id} onValueChange={(v) => applyTheme(v as ThemeId)}>
		<Select.Trigger class="w-44 shrink-0" aria-label="Theme">
			<span
				class="size-4 shrink-0 rounded-full ring-1 ring-black/10"
				style="background:{currentTheme.color}"
			></span>
			<span class="flex-1 truncate text-left">{currentTheme.label}</span>
		</Select.Trigger>
		<Select.Content>
			<Select.Group>
				<Select.GroupHeading>Accent colors</Select.GroupHeading>
				{#each ACCENT_THEMES as t (t.id)}
					<Select.Item value={t.id} label={t.label}>
						<span
							class="size-4 shrink-0 rounded-full ring-1 ring-black/10"
							style="background:{t.color}"
						></span>
						{t.label}
					</Select.Item>
				{/each}
			</Select.Group>
			<Select.Group>
				<Select.GroupHeading>Palettes</Select.GroupHeading>
				{#each PALETTE_THEMES as t (t.id)}
					<Select.Item value={t.id} label={t.label}>
						<span
							class="size-4 shrink-0 rounded-full ring-1 ring-black/10"
							style="background:{t.color}"
						></span>
						{t.label}
					</Select.Item>
				{/each}
			</Select.Group>
		</Select.Content>
	</Select.Root>
{/snippet}

{#snippet accentSwatch()}
	<button
		type="button"
		onclick={() => (pickerOpen = !pickerOpen)}
		aria-label="Choose accent color"
		aria-expanded={pickerOpen}
		class="size-8 cursor-pointer rounded-lg ring-1 ring-black/10 transition-transform hover:scale-105 {pickerOpen ? 'ring-2 ring-primary/60' : ''}"
		style="background:{effective.accent}"
	></button>
{/snippet}

{#snippet accentPicker()}
	<ColorPicker value={effective.accent} onchange={(hex) => setCustom({ accent: hex })} />
{/snippet}

{#snippet tintSlider()}
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
{/snippet}

{#snippet radiusSlider()}
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
{/snippet}





{#snippet layoutSelect()}
	<Select.Root type="single" value={layout.id} onValueChange={(v) => applyLayout(v as LayoutId)}>
		<Select.Trigger class="w-44 shrink-0" aria-label="Layout">
			<span class="flex-1 truncate text-left">
				{LAYOUTS.find((l) => l.id === layout.id)?.label ?? layout.id}
			</span>
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
{/snippet}

{#snippet layoutPreview()}
	<!-- Mini wireframes for each option -->
	<div class="grid grid-cols-5 gap-2">
		{#each LAYOUTS as l (l.id)}
			<button
				type="button"
				onclick={() => applyLayout(l.id)}
				class="flex cursor-pointer flex-col items-center gap-1 rounded-lg border p-2 transition-colors hover:bg-accent/50 {layout.id === l.id ? 'border-primary bg-primary/10 ring-1 ring-primary' : 'border-border bg-card'}"
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
					{:else if l.id === 'poolside'}
						<div class="flex w-full items-center justify-center gap-1 rounded-sm bg-cyan-100/30 py-1">
							<div class="h-3 w-3 rounded-full bg-cyan-400/60"></div>
							<div class="h-3 w-3 rounded-full bg-coral-400/60"></div>
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
{/snippet}

{#snippet fontSelect(key: FontKey, label: string)}
	<Select.Root
		type="single"
		value={isCustomFont[key] ? 'custom' : matchFont(effective[key])}
		onValueChange={(v) => chooseFont(key, v)}
	>
		<Select.Trigger class="w-44 shrink-0" aria-label={label}>
			<span
				class="min-w-0 flex-1 truncate text-left"
				style="font-family:{effective[key]}"
			>
				{isCustomFont[key] ? 'Custom' : familyName(effective[key])}
			</span>
		</Select.Trigger>
		<!-- max-w: a loaded font's name is whatever the file was called, and the dropdown grows to
		     its widest item. -->
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
{/snippet}

{#snippet fontInput(key: FontKey, label: string)}
	<Input
		value={fontName[key]}
		oninput={(e) => typeFont(key, e.currentTarget.value)}
		placeholder="Font installed on this computer, e.g. Inter"
		aria-label="{label} family name"
		spellcheck={false}
		style="font-family:{effective[key]}"
	/>
	{#if fontName[key].trim() && !fontAvailable(fontName[key])}
		<p class="mt-1.5 text-xs text-muted-foreground">
			Not installed — install the font, then reopen settings.
		</p>
	{/if}
{/snippet}

{#snippet lyricsFontSelect()}
	<Select.Root
		type="single"
		value={appearance.lyricsFont}
		onValueChange={(v) => setAppearance({ lyricsFont: v as any })}
	>
		<Select.Trigger class="w-56 shrink-0" aria-label="Lyrics font">
			<span class="min-w-0 flex-1 truncate text-left">
				{LYRIC_FONTS.find((f) => f.id === appearance.lyricsFont)?.label ?? 'System'}
			</span>
		</Select.Trigger>
		<Select.Content class="max-w-64">
			{#each LYRIC_FONTS as f (f.id)}
				<Select.Item value={f.id} label={f.label}>
					<span class="block truncate">{f.label}</span>
				</Select.Item>
			{/each}
		</Select.Content>
	</Select.Root>
{/snippet}

{#snippet addFontButton()}
	<Button variant="outline" size="sm" class="shrink-0" onclick={pickFontFiles}>Add font…</Button>
{/snippet}

{#snippet fontFileList()}
	<div class="flex flex-col gap-1.5">
		{#each custom.fontFiles as path (path)}
			<div class="flex items-center gap-3 rounded-lg bg-secondary/60 py-1.5 pr-1.5 pl-3 text-sm">
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
					class="flex size-6 shrink-0 cursor-pointer items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
				>
					<HugeiconsIcon icon={Cancel01Icon} size={14} />
				</button>
			</div>
		{/each}
	</div>
{/snippet}

{#snippet resetButton()}
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
{/snippet}

{#snippet backdropPicker()}
	{@render segmented(BACKDROP_MODES, backdrop, (id) => setBackdrop(id as BackdropMode))}
{/snippet}

{#snippet packRefreshButton()}
	<Button size="sm" variant="outline" onclick={refreshPacks} disabled={packLoading}>Refresh</Button>
{/snippet}

{#snippet packInstall()}
	<div class="flex gap-2">
		<Input
			placeholder="https://…/pack.zip or id"
			class="flex-1"
			value={packUrl}
			oninput={(e) => (packUrl = e.currentTarget.value)}
		/>
		<Button size="sm" onclick={installPackFromUrl} disabled={packLoading || !packUrl.trim()}>
			{packLoading ? 'Installing…' : 'Install URL'}
		</Button>
		<Button size="sm" variant="outline" onclick={installPackFromZip} disabled={packLoading}>
			From ZIP
		</Button>
	</div>
	{#if packIndex?.packs?.length}
		<div class="mt-3 grid gap-2">
			{#each packIndex.packs as p (p.id)}
				<div class="flex items-center justify-between rounded-lg border px-3 py-2">
					<div class="min-w-0">
						<div class="text-sm font-medium">
							{p.name}
							<span class="text-xs text-muted-foreground">v{p.version}</span>
						</div>
						<div class="truncate text-xs text-muted-foreground">
							{p.description ?? ''} — {p.artist_ids.join(', ')}
						</div>
					</div>
					<Button
						size="sm"
						variant="ghost"
						onclick={() => {
							packUrl = p.url;
							installPackFromUrl();
						}}
					>
						Install
					</Button>
				</div>
			{/each}
		</div>
	{/if}
{/snippet}

{#snippet packList()}
	<div class="flex flex-col gap-1.5">
		{#each artistPacks as ap (ap.id)}
			<div class="flex items-center gap-3 rounded-lg bg-secondary/60 py-1.5 pr-1.5 pl-3">
				<div class="min-w-0 flex-1">
					<div class="truncate text-sm font-medium">
						{ap.name}
						<span class="text-xs text-muted-foreground">{ap.id}</span>
					</div>
					<div class="truncate text-xs text-muted-foreground">
						{ap.artist_ids.join(', ')} {ap.aliases.join(', ')}
					</div>
				</div>
				<Button size="sm" variant="ghost" onclick={() => removePack(ap.id)}>Remove</Button>
			</div>
		{:else}
			<p class="text-sm text-muted-foreground">No packs installed.</p>
		{/each}
	</div>
{/snippet}

<!-- Segmented, not three buttons: the options are one exclusive choice and should look like it. -->
{#snippet segmented(options: { id: string; label: string }[], selected: string, onpick: (id: string) => void)}
	<div class="flex rounded-lg bg-muted p-0.5">
		{#each options as q (q.id)}
			<button
				type="button"
				onclick={() => onpick(q.id)}
				aria-pressed={selected === q.id}
				class="cursor-pointer rounded-md px-3.5 py-1.5 text-xs font-medium transition-colors {selected === q.id ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'}"
			>
				{q.label}
			</button>
		{/each}
	</div>
{/snippet}

{#snippet qualityPicker()}
	{@render segmented(QUALITIES, quality, setQuality)}
{/snippet}

{#snippet downloadQualityPicker()}
	{@render segmented(QUALITIES, downloadQuality, setDownloadQuality)}
{/snippet}

{#snippet downloadFormatPicker()}
	{@render segmented(DOWNLOAD_FORMATS, downloadFormat, setDownloadFormat)}
{/snippet}

{#snippet autoOfflinePicker()}
	{@render segmented(AUTO_OFFLINE_MODES, autoOffline, setAutoOffline)}
{/snippet}

{#snippet crossfadeSlider()}
	<div class="flex w-44 shrink-0 items-center gap-3">
		<Slider
			type="single"
			aria-label="Crossfade seconds"
			min={0}
			max={12}
			step={0.5}
			value={crossfade.secs}
			onValueChange={(v) => setCrossfadeSecs(v)}
		/>
		<span class="w-10 shrink-0 text-right font-mono text-xs text-muted-foreground">
			{crossfade.secs.toFixed(1)}s
		</span>
	</div>
{/snippet}

{#snippet crossfadeMode()}
	<div class="flex gap-2">
		<Button
			size="sm"
			variant={crossfade.mode === 'standard' ? 'default' : 'outline'}
			onclick={() => {
				crossfade.mode = 'standard';
				api.setCrossfade(crossfade.secs, 'standard');
			}}
		>
			Standard
		</Button>
		<Button
			size="sm"
			variant={crossfade.mode === 'smart' ? 'default' : 'outline'}
			onclick={() => {
				crossfade.mode = 'smart';
				api.setCrossfade(crossfade.secs, 'smart');
			}}
		>
			Smart
		</Button>
	</div>
{/snippet}

{#snippet sleepTimerPresets()}
	<div class="flex flex-wrap gap-2">
		<Button
			size="sm"
			variant={sleepTimer.mode === 'off' ? 'default' : 'outline'}
			onclick={() => setSleepTimer('off')}
		>
			Off
		</Button>
		<Button
			size="sm"
			variant={sleepTimer.mode === 'end_of_song' ? 'default' : 'outline'}
			onclick={() => setSleepTimer('end_of_song')}
		>
			End of song
		</Button>
		{#each [15, 30, 60] as m (m)}
			<Button
				size="sm"
				variant={sleepTimer.mode === 'minutes' ? 'default' : 'outline'}
				onclick={() => setSleepTimer('minutes', m)}
			>
				{m} min
			</Button>
		{/each}
	</div>
{/snippet}

{#snippet ytdlpInstall()}
	<div>
		<Button size="sm" variant="outline" onclick={installYtdlp}>Install now</Button>
	</div>
{/snippet}

{#snippet clientList()}
	<p class="mb-3 max-w-prose text-xs leading-relaxed text-muted-foreground">
		Advanced — turn a client off to skip it when resolving streams. Overridden by the
		<span class="font-mono">LIMUSIC_DISABLED_CLIENTS</span> env var.
	</p>
	<div class="flex flex-col gap-2">
		{#each clients as name (name)}
			<div class="flex items-center justify-between rounded-lg bg-muted/60 py-1.5 pr-2 pl-3">
				<span class="font-mono text-xs">{name}</span>
				<Switch checked={!disabled.has(name)} onCheckedChange={() => toggleClient(name)} />
			</div>
		{/each}
	</div>
{/snippet}

{#snippet downloadDirForm()}
	<div class="flex items-center gap-2">
		<Input class="min-w-0 flex-1" readonly value={downloadDir} placeholder="App data / downloads" />
		<Button size="sm" variant="outline" onclick={pickDownloadDir}>Browse…</Button>
	</div>
{/snippet}

{#snippet clearAllButton()}
	<Button
		size="sm"
		variant="ghost"
		disabled={downloads.length === 0}
		onclick={clearAllDownloads}
	>
		Clear all
	</Button>
{/snippet}

{#snippet downloadsList()}
	{#if downloads.length === 0}
		<p class="text-sm text-muted-foreground">
			Nothing saved yet. Use the ⋮ menu on any track and choose “Download”.
		</p>
	{:else}
		<div class="flex flex-col gap-1.5">
			{#each downloads as d (d.video_id)}
				<div class="flex items-center justify-between gap-3 rounded-lg border px-3 py-2">
					<div class="min-w-0">
						<div class="truncate text-sm font-medium">{d.title}</div>
						<div class="truncate text-xs text-muted-foreground">{d.artists}</div>
					</div>
					<div class="flex shrink-0 items-center gap-3 text-xs text-muted-foreground">
						<span class="uppercase">{d.format}</span>
						<span>{fmtSize(d.size_bytes)}</span>
						<Button size="sm" variant="ghost" onclick={() => removeDownload(d.video_id)}>
							Remove
						</Button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
{/snippet}

{#snippet proxyForm()}
	<form
		class="flex gap-2"
		onsubmit={(e) => {
			e.preventDefault();
			saveProxy();
		}}
	>
		<Input bind:value={proxyInput} placeholder="http://host:port (blank = none)" class="min-w-0 flex-1" />
		<Button type="submit" variant="outline">Save</Button>
	</form>
{/snippet}

{#snippet clearButton()}
	<Button variant="destructive" size="sm" onclick={doClearCaches} disabled={clearing}>
		{clearing ? 'Clearing…' : 'Clear caches'}
	</Button>
{/snippet}

{#snippet updateButton()}
	{#if updateState.available && !updateState.canInstall}
		<Button size="sm" onclick={openDownloadPage}>Download</Button>
	{:else if updateState.available}
		<Button size="sm" onclick={installUpdate} disabled={updateState.installing}>
			{updateState.installing ? 'Updating…' : 'Update now'}
		</Button>
	{:else}
		<Button variant="outline" size="sm" onclick={checkUpdates} disabled={updateState.checking}>
			{updateState.checking ? 'Checking…' : 'Check for updates'}
		</Button>
	{/if}
{/snippet}

{#snippet updateAlert()}
	<Alert variant={updateResult?.error ? 'destructive' : 'default'}>
		<AlertDescription>{updateResult?.message}</AlertDescription>
	</Alert>
{/snippet}
