// Independent finite PCND1 reader. No product parser, geometry or renderer import.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import { block } from './main-rotation-browser-codecs.mjs';
export const sha = (b) => createHash('sha256').update(b).digest('hex');
const caseBytes = fs.readFileSync(
	new URL('./main-rotation-browser-cases.json', import.meta.url),
);
assert(caseBytes.length < 16384);
const facts = JSON.parse(caseBytes);
export const cases = facts.cases;
const utf8 = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
class Reader {
	constructor(bytes) {
		this.bytes = bytes;
		this.pos = 0;
	}
	take(n) {
		assert(
			Number.isSafeInteger(n) &&
				n >= 0 &&
				this.pos + n <= this.bytes.length,
		);
		const b = this.bytes.subarray(this.pos, this.pos + n);
		this.pos += n;
		return b;
	}
	byte() {
		return this.take(1)[0];
	}
	u(max) {
		let n = 0;
		for (let i = 0; i < 3; i++) {
			const b = this.byte();
			n += (b & 127) * 2 ** (i * 7);
			if (b < 128) {
				assert((i === 0 || b !== 0) && n <= max);
				return n;
			}
		}
		throw Error('finite canonical integer required');
	}
	id() {
		const id = this.take(16).toString('hex');
		assert.notEqual(id, '00'.repeat(16));
		return id;
	}
	text() {
		return utf8.decode(this.take(this.u(64)));
	}
}
export function fixture() {
	const bytes = Buffer.from(facts.scene.source_hex, 'hex');
	assert.equal(bytes.length, facts.scene.source_bytes);
	assert.equal(sha(bytes), facts.scene.source_sha256);
	return { bytes, sha256: sha(bytes), scene: facts.scene };
}
export function inspect(value, phase) {
	assert(Object.hasOwn(cases, phase));
	assert(
		value instanceof Uint8Array &&
			value.byteLength <= 65536 &&
			value.byteLength >= 43,
	);
	const bytes = Buffer.from(value),
		expected = cases[phase],
		[width, height] = expected.canvas;
	assert.equal(bytes.subarray(0, 8).toString('hex'), '50434e44010d0a1a');
	assert.equal(
		sha(bytes.subarray(0, -32)),
		bytes.subarray(-32).toString('hex'),
	);
	const r = new Reader(bytes.subarray(8, -32));
	assert.equal(r.id(), facts.scene.document);
	const state = r.id();
	if (phase === 'baseline') assert.equal(state, facts.scene.state);
	assert.deepEqual([r.u(3), r.u(3)], [width, height]);
	assert.equal(r.byte(), 0, 'no title/metadata/filter/mask/style profile');
	assert.equal(r.u(1), 1);
	assert.equal(r.id(), facts.scene.layer);
	assert.equal(r.text(), facts.scene.name);
	assert.equal(r.byte(), 1);
	assert.equal(r.byte(), 255);
	assert.deepEqual([r.u(0), r.u(0)], [0, 0]);
	assert.deepEqual([r.u(3), r.u(3)], [width, height]);
	assert.deepEqual([...r.take(4)], facts.scene.default);
	let rgba = Buffer.alloc(width * height * 4),
		tiles = 0;
	for (;;) {
		const kind = r.byte();
		if (kind === 0) break;
		assert(
			[1, 2, 4, 5].includes(kind),
			'finite portable writer codecs only',
		);
		assert(++tiles <= 1);
		assert.deepEqual([r.u(0), r.u(0)], [0, 0]);
		const n = r.u(65536);
		assert(n > 0);
		rgba = block(kind, r.take(n), width * height * 4);
	}
	assert.equal(r.pos, r.bytes.length, 'complete sealed finite document only');
	assert.deepEqual(
		[...rgba],
		expected.rgba,
		`every ${phase} straightRGBA byte`,
	);
	assert.equal(sha(rgba), expected.rgba_sha256);
	return {
		state,
		canvas: [width, height],
		sha256: sha(bytes),
		bytes: bytes.length,
		rgba_sha256: sha(rgba),
		pixels: width * height,
		coefficients: expected.coefficients,
	};
}
