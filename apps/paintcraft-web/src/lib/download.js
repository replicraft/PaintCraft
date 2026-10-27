// A download requests a browser-managed copy; it cannot confirm persistence.
export function downloadCopy(bytes, name, mime) {
	const url = URL.createObjectURL(new Blob([bytes], { type: mime }));
	const link = document.createElement('a');
	link.href = url;
	link.download = name;
	link.hidden = true;
	document.body.append(link);
	try {
		link.click();
	} finally {
		link.remove();
		// Let the browser consume the URL before releasing the encoded data.
		setTimeout(() => URL.revokeObjectURL(url), 1000);
	}
}

// Advanced admitted exports retain their lease until the URL handle retires.
// Revocation is not confirmed persistence or browser-memory reclamation.
export function downloadCopyTracked(bytes, name, mime) {
	let url,
		link,
		requested = false;
	try {
		url = URL.createObjectURL(new Blob([bytes], { type: mime }));
		link = document.createElement('a');
		link.href = url;
		link.download = name;
		link.hidden = true;
		document.body.append(link);
		link.click();
		requested = true;
	} catch {
		// Cleanup below also handles a refused or throwing click.
	}
	try {
		link?.remove();
	} catch {
		/* The URL still must be retired. */
	}
	if (url === undefined) return Promise.resolve({ requested: false });
	return new Promise((resolve, reject) => {
		setTimeout(
			() => {
				try {
					URL.revokeObjectURL(url);
					resolve({ requested });
				} catch (error) {
					reject(error);
				}
			},
			requested ? 1000 : 0,
		);
	});
}
