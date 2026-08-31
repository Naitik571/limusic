// Mock Tauri's convertFileSrc (no-op for browser)
const convertFileSrc = (path: string) => path;

// Rewrite a Google image URL to (about) the pixel size a slot actually renders, so the webview
// doesn't decode a 544px (or 1080p) image for a 40px row. Only lh3/yt3 googleusercontent-style
// URLs carry the size in the URL (`=w544-h544` / `=s576` suffixes); anything else (notably
// i.ytimg.com path-variant thumbs, where other sizes can 404) is returned unchanged.

// InnerTube hands some artwork back protocol-relative ("//lh3.googleusercontent.com/…") —
// Liked Music and Episodes for Later do exactly this. Those are https on Google's CDN; left
// alone they'd resolve against the app's own origin (and CSP), so pin them before anything
// else looks at the leading "/" and mistakes them for local paths.
function absolute(url: string): string {
	return url.startsWith('//') ? `https:${url}` : url;
}

export function thumb(url: string | undefined | null, px: number): string | undefined {
	if (!url) return undefined;
	const u = absolute(url);
	// Local library artwork is a path on this machine, not a URL. The webview can't open a bare
	// path, so hand it through Tauri's asset protocol. Kept here rather than at the command
	// boundary so what gets stored (queue, Shortcuts) stays the real path — which is also what
	// SMTC needs.
	if (u.startsWith('/') || /^[A-Za-z]:[\\/]/.test(u)) return convertFileSrc(u);
	if (/=w\d+-h\d+/.test(u)) return u.replace(/=w\d+-h\d+/, `=w${px}-h${px}`);
	if (/=s\d+/.test(u)) return u.replace(/=s\d+/, `=s${px}`);
	return u;
}

// Highest practical size for a slot that is *about* the art (now-playing hero, floating card):
// the 1080 token on googleusercontent URLs, `maxresdefault` on i.ytimg path-variants. The
// latter is a guess for some videos (no maxres frame exists) — callers pair this with the
// same onerror step-down the rest of the app uses.
export function thumbHQ(url: string | undefined | null): string | undefined {
	if (!url) return undefined;
	const u = absolute(url);
	if (u.startsWith('/') || /^[A-Za-z]:[\\/]/.test(u)) return convertFileSrc(u);
	if (/=w\d+-h\d+/.test(u)) return u.replace(/=w\d+-h\d+/, '=w1080-h1080');
	if (/=s\d+/.test(u)) return u.replace(/=s\d+/, '=s1080');
	if (u.includes('hqdefault')) return u.replace('hqdefault', 'maxresdefault');
	return u;
}
