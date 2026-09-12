// Auto-update via Tauri's updater plugin. Checks a signed latest.json on GitHub Releases; the
// startup check is silent unless an update exists, the Settings check always reports a result.
// Only self-updates the AppImage build on Linux (Tauri limitation) — .deb, .rpm and distro packages
// update through their package manager, so they get a download link instead. See `canInstall`.
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { toast } from './player.svelte';
import { canSelfUpdate, getSettings, openExternal } from './api';

const RELEASES_URL = 'https://github.com/SimoHypers/limusic/releases/latest';

export const updateState = $state({
	available: null as { version: string } | null, // set when a newer version is waiting
	canInstall: true, // false on packaged Linux builds; always resolved before `available` is set
	checking: false, // Settings "Check for updates" is in flight
	installing: false, // downloading/installing the update
	progress: 0, // 0..1 download fraction while installing
	downloaded: 0, // bytes so far (for the tooltip)
	total: 0, // bytes total (0 = unknown)
	failed: null as string | null // last install failure, with Retry beside it
});

// The resolved handle to download; kept out of reactive state (it's not serializable/renderable).
let pending: Update | null = null;

async function look(): Promise<boolean> {
	const u = await check();
	if (u) {
		pending = u;
		// Before `available`, so the banner never renders with the wrong button for a frame. On the
		// (unlikely) IPC failure, fall back to the download link: it works everywhere, while
		// "Update now" on a packaged build does not.
		updateState.canInstall = await canSelfUpdate().catch(() => false);
		updateState.available = { version: u.version };
		return true;
	}
	// No update: drop any stale handle so Install can't download a superseded build.
	pending = null;
	return false;
}

/** On app open: show the update toast if one exists, stay silent otherwise. */
export async function checkForUpdatesQuiet() {
	try {
		await look();
	} catch (e) {
		console.error('update check failed', e); // no endpoint / offline — don't nag on launch
	}
}

/** From Settings: return the outcome so the modal can show it inline (a toast renders behind the
 *  dialog). `error` picks the Alert variant. */
export async function checkForUpdatesInteractive(): Promise<{ message: string; error: boolean }> {
	updateState.checking = true;
	try {
		if (await look())
			return { message: `Update available: v${updateState.available!.version}`, error: false };
		return { message: 'You are running the latest version', error: false };
	} catch (e) {
		return { message: `Update check failed: ${e}`, error: true };
	} finally {
		updateState.checking = false;
	}
}

/** Send a packaged build to the releases page. Their package manager does the actual updating; all
 *  the app can do is say a new version exists and get out of the way. */
export function openDownloadPage() {
	openExternal(RELEASES_URL).catch((e) => toast.error(`Couldn't open the browser: ${e}`));
}

/** Download + install the pending update, then relaunch into the new version. Progress
 *  streams into `updateState` for the banner bar; failures stay visible with Retry. */
export async function installUpdate() {
	if (!pending || updateState.installing) return;
	updateState.installing = true;
	updateState.failed = null;
	updateState.progress = 0;
	updateState.downloaded = 0;
	updateState.total = 0;
	try {
		await pending.downloadAndInstall((e) => {
			if (e.event === 'Started') {
				updateState.total = e.data.contentLength ?? 0;
			} else if (e.event === 'Progress') {
				updateState.downloaded += e.data.chunkLength;
				if (updateState.total > 0) {
					updateState.progress = Math.min(1, updateState.downloaded / updateState.total);
				}
			} else if (e.event === 'Finished') {
				updateState.progress = 1;
			}
		});
		await relaunch();
	} catch (e) {
		updateState.failed = String(e);
		updateState.installing = false;
	}
}

/** Dismiss a failed install back to the plain banner (Retry lives beside the error). */
export function dismissUpdateFailure() {
	updateState.failed = null;
	updateState.installing = false;
	updateState.progress = 0;
}
