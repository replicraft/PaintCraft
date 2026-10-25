import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import {
	ASSET_LIMIT,
	BINDGEN_VERSION,
	checkAssets,
	copyFonts,
} from './assets.mjs';
import { loadCjkFonts } from '../src/lib/fonts.js';

test('Pages limit checks actual uncompressed bytes and nested assets', () => {
	const directory = fs.mkdtempSync(
		path.join(os.tmpdir(), 'paintcraft-assets-'),
	);
	try {
		fs.mkdirSync(path.join(directory, 'nested'));
		const file = path.join(directory, 'nested/editor.wasm');
		const fd = fs.openSync(file, 'w');
		fs.ftruncateSync(fd, ASSET_LIMIT);
		fs.closeSync(fd);
		assert.deepEqual(checkAssets(directory), {
			'nested/editor.wasm': ASSET_LIMIT,
		});
		fs.appendFileSync(file, '\0');
		assert.throws(
			() => checkAssets(directory),
			/nested\/editor.wasm.*Cloudflare Pages/,
		);
	} finally {
		fs.rmSync(directory, { recursive: true, force: true });
	}
});
test('font assets are exact copies of shared root files with licenses and matching hashes', () => {
	const directory = fs.mkdtempSync(
		path.join(os.tmpdir(), 'paintcraft-fonts-'),
	);
	try {
		const root = fileURLToPath(new URL('../../../', import.meta.url));
		const manifest = copyFonts(root, directory);
		for (const [name, family] of [
			['japanese', 'noto-sans-jp'],
			['chinese', 'noto-sans-sc'],
		]) {
			const entry = manifest[name],
				bytes = fs.readFileSync(path.join(directory, entry.file));
			assert.equal(bytes.length, entry.bytes);
			assert.equal(
				createHash('sha256').update(bytes).digest('hex'),
				entry.sha256,
			);
			assert.ok(
				fs.existsSync(path.join(directory, `${family}-LICENSE.txt`)),
			);
		}
		for (const family of ['selawik', 'noto-sans-devanagari'])
			assert.ok(
				fs.existsSync(path.join(directory, `${family}-LICENSE.txt`)),
			);
		checkAssets(directory);
	} finally {
		fs.rmSync(directory, { recursive: true, force: true });
	}
});
test('browser font loading rejects failed responses, wrong lengths and corrupt payloads', async () => {
	const original = globalThis.fetch;
	const bytes = new Uint8Array(32);
	bytes.set([79, 84, 84, 79]);
	const sha256 = createHash('sha256').update(bytes).digest('hex');
	const entry = { file: `font.${sha256}.otf`, bytes: bytes.length, sha256 };
	const base = new URL('https://paintcraft.test/');
	try {
		for (const mode of ['valid', 'failed', 'length', 'corrupt']) {
			globalThis.fetch = async (url) => {
				if (url.pathname.endsWith('manifest.json'))
					return Response.json({ japanese: entry, chinese: entry });
				if (mode === 'failed') return new Response('', { status: 404 });
				const data = bytes.slice();
				if (mode === 'corrupt') data[31] = 1;
				return new Response(mode === 'length' ? data.slice(1) : data);
			};
			if (mode === 'valid')
				assert.deepEqual(await loadCjkFonts(base), [bytes, bytes]);
			else
				await assert.rejects(
					loadCjkFonts(base),
					/font: HTTP|font size mismatch|font checksum mismatch/,
				);
		}
	} finally {
		globalThis.fetch = original;
	}
});

test('binding tool matches the locked Rust ABI', () => {
	const lock = fs.readFileSync(
		new URL('../../../Cargo.lock', import.meta.url),
		'utf8',
	);
	const version = lock.match(
		/\[\[package\]\]\s+name = "wasm-bindgen"\s+version = "([^\"]+)"/,
	)[1];
	assert.equal(BINDGEN_VERSION, version);
});
