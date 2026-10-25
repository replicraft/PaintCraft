// Keep menu state tied to the actual DOM, including browser-initiated exits.
export function fullscreenActive(doc = globalThis.document) {
	return Boolean(
		doc?.documentElement && doc.fullscreenElement === doc.documentElement,
	);
}

// Await the native promises so permission failures never become unhandled rejections.
export async function toggleFullscreen(doc = globalThis.document) {
	if (!doc?.documentElement) throw new Error('No browser document');
	if (fullscreenActive(doc)) {
		await doc.exitFullscreen();
	} else {
		if (doc.fullscreenEnabled === false)
			throw new Error('Fullscreen unavailable');
		await doc.documentElement.requestFullscreen();
	}
	return fullscreenActive(doc);
}
