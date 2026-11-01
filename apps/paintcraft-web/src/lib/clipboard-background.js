// Permission/user activation first; real PNG bytes arrive later from Rust Worker.
let active = false;
export function beginDeferredPngClipboard() {
	if (
		active ||
		!globalThis.isSecureContext ||
		typeof globalThis.navigator?.clipboard?.write !== 'function' ||
		typeof ClipboardItem !== 'function'
	)
		throw Error('Clipboard unavailable or busy');
	let resolve,
		reject,
		settled = false;
	const payload = new Promise((yes, no) => {
		resolve = yes;
		reject = no;
	});
	// A browser may reject write() before consuming the supplied Blob promise.
	// Observe payload rejection independently, while passing the original promise
	// to ClipboardItem so rejection still propagates to the write acknowledgement.
	payload.catch(() => {});
	active = true;
	let written;
	try {
		written = navigator.clipboard.write([
			new ClipboardItem({ 'image/png': payload }),
		]);
	} catch (error) {
		active = false;
		reject(error);
		payload.catch(() => {});
		throw error;
	}
	const promise = Promise.resolve(written).finally(() => {
		active = false;
	});
	// Awaited by Rust only after it receives bytes; early browser rejection must
	// not emit an unhandled rejection while the admitted Worker is still running.
	promise.catch(() => {});
	return Object.freeze({
		promise,
		resolve(bytes) {
			if (
				settled ||
				!(bytes instanceof Uint8Array) ||
				bytes.length < 8 ||
				bytes.length > 16 * 1024 * 1024 ||
				![137, 80, 78, 71, 13, 10, 26, 10].every(
					(v, i) => bytes[i] === v,
				)
			)
				throw Error('Clipboard PNG invalid');
			settled = true;
			resolve(new Blob([bytes], { type: 'image/png' }));
		},
		cancel() {
			if (!settled) {
				settled = true;
				reject(Error('Clipboard cancelled'));
			}
		},
	});
}
