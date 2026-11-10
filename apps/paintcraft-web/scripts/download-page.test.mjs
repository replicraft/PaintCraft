import assert from 'node:assert/strict';
import test from 'node:test';

let sequence = 0;
const asset = (name, state = 'uploaded', size = 42) => ({ name, state, size });
const release = (version, extra = []) => ({
	tag_name: `v${version}`,
	assets: [asset(`paintcraft-${version}-linux-x64.AppImage`), ...extra],
});
const settle = () => new Promise((resolve) => setImmediate(resolve));

async function page(t, responses, navigator = { platform: 'Linux x86_64' }) {
	const events = [];
	const browser = {
		primary: true,
		classList: {
			remove: () => {
				browser.primary = false;
			},
		},
	};
	const primary = {
		hidden: true,
		click() {
			events.push(this.href);
		},
	};
	const retry = {
		hidden: true,
		addEventListener(_name, action) {
			this.action = action;
		},
	};
	const status = { textContent: '' };
	const version = { textContent: 'v0.1.1' };
	const link = (architecture, suffix, optional = false) => {
		const row = optional ? { hidden: true } : null;
		const attributes = new Map();
		return {
			dataset: { target: `linux-${architecture}`, suffix },
			href: optional ? undefined : 'https://example.test/old-package',
			row,
			closest: () => row,
			removeAttribute(name) {
				if (name === 'href') this.href = undefined;
				attributes.delete(name);
			},
			setAttribute(name, value) {
				attributes.set(name, value);
			},
		};
	};
	const links = [
		link('x64', '.AppImage'),
		link('x64', '.flatpak', true),
		link('arm64', '.flatpak', true),
	];
	const document = {
		addEventListener() {},
		getElementById(id) {
			return {
				'download-status': status,
				'recommended-download': primary,
				'retry-download': retry,
				'release-version': version,
			}[id];
		},
		querySelectorAll: () => links,
		querySelector: () => browser,
	};
	for (const [name, value] of Object.entries({ document, navigator })) {
		const before = Object.getOwnPropertyDescriptor(globalThis, name);
		Object.defineProperty(globalThis, name, { configurable: true, value });
		t.after(() =>
			before
				? Object.defineProperty(globalThis, name, before)
				: delete globalThis[name],
		);
	}
	t.mock.method(globalThis, 'fetch', async () => {
		const value = responses.shift();
		if (value instanceof Error) throw value;
		return { ok: true, json: async () => value };
	});
	await import(`../src/lib/download-page.js?flatpak-test=${sequence++}`);
	await settle();
	return { links, primary, browser, retry, events, status };
}

test('old published releases keep both Flatpak rows hidden and have no dead fallback URLs', async (t) => {
	const view = await page(t, [release('0.1.1')]);
	for (const link of view.links.slice(1)) {
		assert.equal(link.row.hidden, true);
		assert.equal(link.href, undefined);
	}
	assert.equal(view.events.length, 1);
	assert.ok(view.events[0].endsWith('/paintcraft-0.1.1-linux-x64.AppImage'));
});

test('only an uploaded exact-architecture Flatpak is revealed; retry removes stale availability', async (t) => {
	const view = await page(t, [
		release('0.2.0', [
			asset('paintcraft-0.2.0-linux-x64.flatpak'),
			asset('paintcraft-0.2.0-linux-arm64.flatpak', 'uploading'),
		]),
		release('0.2.0'),
	]);
	assert.equal(view.links[1].row.hidden, false);
	assert.ok(
		view.links[1].href.endsWith('/paintcraft-0.2.0-linux-x64.flatpak'),
	);
	assert.equal(view.links[2].row.hidden, true);
	assert.equal(view.links[2].href, undefined);
	assert.ok(view.events[0].endsWith('.AppImage'));
	view.retry.action();
	await settle();
	assert.equal(view.links[1].row.hidden, true);
	assert.equal(view.links[1].href, undefined);
});

test('a failed release API request never exposes speculative Flatpak fallback links', async (t) => {
	const view = await page(t, [new Error('offline')]);
	for (const link of view.links.slice(1)) {
		assert.equal(link.row.hidden, true);
		assert.equal(link.href, undefined);
	}
	assert.equal(view.primary.hidden, true);
	assert.equal(view.browser.primary, true);
	assert.equal(view.events.length, 0);
	assert.equal(view.retry.hidden, false);
});

test('FreeBSD keeps the browser primary even when desktop Flatpaks are available', async (t) => {
	const view = await page(
		t,
		[release('0.2.0', [asset('paintcraft-0.2.0-linux-x64.flatpak')])],
		{ platform: 'FreeBSD amd64', userAgent: 'X11' },
	);
	assert.equal(view.links[1].row.hidden, false);
	assert.equal(view.primary.hidden, true);
	assert.equal(view.browser.primary, true);
	assert.equal(view.events.length, 0);
});
