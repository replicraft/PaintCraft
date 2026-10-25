// One owned print document. No popup, editor markup or user-supplied HTML.
export class BrowserPrinter {
	constructor(doc = globalThis.document, urls = globalThis.URL) {
		this.document = doc;
		this.urls = urls;
		this.active = null;
	}

	print(bytes, title) {
		if (this.active) return Promise.reject(new Error('Printing is busy'));
		if (!this.document?.body)
			return Promise.reject(new Error('No browser document'));
		return new Promise((resolve, reject) => {
			const focus = this.document.activeElement;
			const frame = this.document.createElement('iframe');
			frame.title = title;
			frame.setAttribute('aria-hidden', 'true');
			frame.style.cssText =
				'position:fixed;left:-10000px;top:0;width:800px;height:1000px;border:0';
			let url;
			let image;
			let started = false;
			const finish = (outcome, error) => {
				if (this.active !== job) return;
				this.active = null;
				clearTimeout(job.timer);
				frame.contentWindow?.removeEventListener('afterprint', closed);
				frame.contentWindow?.removeEventListener('beforeprint', before);
				frame.onload = null;
				if (image) image.src = '';
				frame.remove();
				focus?.focus();
				if (url) this.urls.revokeObjectURL(url);
				if (error) reject(error);
				else resolve(outcome);
			};
			const closed = () => finish('closed');
			const before = () => {
				started = true;
				clearTimeout(job.timer);
			};
			const job = { cancel: () => finish('cancelled'), timer: null };
			this.active = job;
			job.timer = setTimeout(
				() =>
					finish(null, new Error('Print image did not become ready')),
				30000,
			);
			frame.onload = async () => {
				frame.onload = null;
				try {
					const doc = frame.contentDocument;
					const win = frame.contentWindow;
					if (!doc?.body || typeof win?.print !== 'function')
						throw new Error('Browser printing unavailable');
					doc.title = title;
					const style = doc.createElement('style');
					style.textContent = `
@page { size: auto; margin: 10mm; }
html, body { margin: 0; padding: 0; background: white; }
body { height: 100vh; display: grid; place-items: center; }
img { display: block; max-width: 100%; max-height: 100vh; object-fit: contain; break-inside: avoid; }
`;
					doc.head.append(style);
					image = doc.createElement('img');
					image.alt = title;
					url = this.urls.createObjectURL(
						new Blob([bytes], { type: 'image/png' }),
					);
					image.src = url;
					doc.body.append(image);
					await image.decode();
					if (this.active !== job) return;
					win.addEventListener('beforeprint', before);
					win.addEventListener('afterprint', closed);
					win.print();
					focus?.focus();
					// The HTML printing steps fire beforeprint before returning, even
					// when the dialog itself is asynchronous. Silent denial has no event.
					if (!started)
						throw new Error(
							'Browser did not accept the print request',
						);
				} catch (error) {
					finish(null, error);
				}
			};
			try {
				this.document.body.append(frame);
			} catch (error) {
				finish(null, error);
			}
		});
	}

	cancel() {
		this.active?.cancel();
	}
}

const printer = new BrowserPrinter();
export function printImage(bytes, title) {
	return printer.print(bytes, title);
}
export function cancelPrint() {
	printer.cancel();
}
