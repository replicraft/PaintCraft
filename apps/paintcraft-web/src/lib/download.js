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
