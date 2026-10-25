import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
	listLocalFonts,
	loadLocalFamily,
	localFontsSupported,
} from '../src/lib/local-fonts.js';

test('installed fonts require permission, enumerate families and load real face bytes lazily', async () => {
	let queries = 0;
	let reads = 0;
	let granted = false;
	const face = (family, bytes) => ({
		family,
		async blob() {
			reads++;
			return new Blob([new Uint8Array(bytes)]);
		},
	});
	globalThis.window = {
		async queryLocalFonts() {
			queries++;
			return [
				face('Arial', [1, 2]),
				face('Arial', [3]),
				face('Other', [4]),
			];
		},
	};
	Object.defineProperty(globalThis, 'navigator', {
		configurable: true,
		value: {
			permissions: {
				query: async () => ({ state: granted ? 'granted' : 'prompt' }),
			},
		},
	});
	assert.equal(localFontsSupported(), true);
	assert.deepEqual(await listLocalFonts(false), []);
	assert.equal(queries, 0);
	assert.deepEqual(await listLocalFonts(true), ['Arial', 'Other']);
	assert.equal(reads, 0);
	assert.deepEqual(await loadLocalFamily('Arial'), [
		new Uint8Array([1, 2]),
		new Uint8Array([3]),
	]);
	assert.equal(reads, 2);
	granted = true;
	assert.deepEqual(await listLocalFonts(false), ['Arial', 'Other']);
	await assert.rejects(loadLocalFamily('Missing'), /no longer available/);
	window.queryLocalFonts = async () => [
		{ family: 'Huge', blob: async () => ({ size: 33 * 1024 * 1024 }) },
	];
	await listLocalFonts(true);
	await assert.rejects(loadLocalFamily('Huge'), /limit/);
	window.queryLocalFonts = async () => {
		throw new DOMException('Denied', 'NotAllowedError');
	};
	await assert.rejects(listLocalFonts(true), { name: 'NotAllowedError' });
	delete window.queryLocalFonts;
	assert.equal(localFontsSupported(), false);
	assert.deepEqual(await listLocalFonts(true), []);
});
