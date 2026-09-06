// Press an uploaded photo onto a vinyl disc (BlazePod-style compositing): the photo is
// downscaled to 512px and a procedural groove texture is laid over it — concentric cuts
// plus sparse light-catch arcs — so a custom cover reads as a pressed record, not a photo
// in a circle. Output is JPEG (localStorage quota) and any failure falls back to the raw
// upload, never a broken cover.
export function pressVinyl(file: File): Promise<string> {
	const raw = new Promise<string>((resolve, reject) => {
		const rd = new FileReader();
		rd.onload = () => resolve(String(rd.result));
		rd.onerror = () => reject(new Error('read'));
		rd.readAsDataURL(file);
	});
	return raw.then(
		(dataUrl) =>
			new Promise<string>((resolve) => {
				const img = new Image();
				img.onload = () => {
					try {
						const S = 512;
						const c = document.createElement('canvas');
						c.width = S;
						c.height = S;
						const ctx = c.getContext('2d');
						if (!ctx) {
							resolve(dataUrl);
							return;
						}
						// Cover-draw: fill the square, center-cropped.
						const s = Math.max(S / img.width, S / img.height);
						const w = img.width * s, h = img.height * s;
						ctx.drawImage(img, (S - w) / 2, (S - h) / 2, w, h);
						// Groove cuts: dark hairlines every ~4px from the label edge out.
						ctx.strokeStyle = 'rgba(0,0,0,0.10)';
						ctx.lineWidth = 1;
						for (let r = 70; r < 256; r += 4) {
							ctx.beginPath();
							ctx.arc(S / 2, S / 2, r, 0, Math.PI * 2);
							ctx.stroke();
						}
						// Light-catch arcs: a few bright partial rings, like a sheen frozen in.
						ctx.strokeStyle = 'rgba(255,255,255,0.10)';
						ctx.lineWidth = 7;
						for (let k = 0; k < 5; k++) {
							const r = 90 + k * 32;
							const a0 = 0.6 + k * 0.9;
							ctx.beginPath();
							ctx.arc(S / 2, S / 2, r, a0, a0 + 0.7);
							ctx.stroke();
						}
						resolve(c.toDataURL('image/jpeg', 0.85));
					} catch {
						resolve(dataUrl);
					}
				};
				img.onerror = () => resolve(dataUrl);
				img.src = dataUrl;
			})
	).catch(() => '');
}
