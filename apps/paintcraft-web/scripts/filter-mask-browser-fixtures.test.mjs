import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { fixture, assertSaved } from './lib/filter-mask-browser-fixtures.mjs';
const hash = (b) => createHash('sha256').update(b).digest();
const vectors = JSON.parse(
	fs.readFileSync(
		new URL('./fixtures/filter-mask-saved-vectors.json', import.meta.url),
	),
).cases;
const edit = (b, fn) => {
	const body = Buffer.from(b.subarray(0, -32));
	fn(body);
	return Buffer.concat([body, hash(body)]);
};
test('independent full RGBA distinguishes filter mask from unchanged own coverage', () => {
	const base = fixture();
	for (const row of vectors) {
		const bytes = Buffer.from(row.hex, 'hex');
		assert.equal(hash(bytes).toString('hex'), row.sha256);
		const saved = assertSaved(bytes, { mask: row.mask });
		assert.deepEqual([...saved.rgba], row.rgba);
		assert.deepEqual(saved.rawIdentity, base.rawIdentity);
		assert.deepEqual(saved.ownMask, base.ownMask);
		assert.equal(saved.filters[0].id, base.filters[0].id);
		assert.notEqual(saved.state, base.state);
	}
});
test('fresh byte/semantic owners and offset input views cannot corrupt later calls', () => {
	const a = fixture(),
		expected = fixture();
	a.bytes.fill(0);
	a.ownMask.coverage.fill(0);
	a.filters[0].id = 'wrong';
	a.rgba.fill(0);
	assert.deepEqual(fixture(), expected);
	const b = Buffer.from(vectors[1].hex, 'hex');
	const padded = Buffer.concat([Buffer.from([8]), b, Buffer.from([9])]);
	assert.deepEqual(
		assertSaved(
			new Uint8Array(padded.buffer, padded.byteOffset + 1, b.length),
			{ mask: 'selection' },
		),
		assertSaved(b, { mask: 'selection' }),
	);
});
test('source identity/filter UUID/raw color/own mask and nil state mutations refuse', () => {
	const base = fixture().bytes;
	for (const fn of [
		(b) => b.fill(0, 50, 66),
		(b) => {
			b[34] ^= 1;
		},
		(b) => {
			b[b.indexOf(Buffer.from('a4'.repeat(16), 'hex'))] ^= 1;
		},
		(b) => {
			b[b.indexOf(Buffer.from([64, 128, 192, 255]))] ^= 1;
		},
		(b) => {
			b[b.length - 2] = 128;
		},
		(b) => {
			b[0] |= 128;
		},
	])
		assert.throws(() => assertSaved(edit(base, fn), { mask: 'none' }));
	for (const bad of [
		base.subarray(1),
		Buffer.concat([base, Buffer.from([0])]),
		new Uint8Array(4097),
		[],
	])
		assert.throws(() => assertSaved(bad, { mask: 'none' }));
});
test('filter mask side, enabled state and exact own mask are independent requirements', () => {
	const b = Buffer.from(vectors[1].hex, 'hex');
	assert.throws(() => assertSaved(b, { mask: 'none' }));
	assert.throws(() => assertSaved(b, { mask: 'disabled' }));
	const needle = Buffer.from([1, 1, 1, 0, 0, 8, 4, 1, 0, 1, 1, 1, 1, 255]);
	const at = b.indexOf(needle);
	assert(at >= 0);
	for (const [offset, value] of [
		[2, 2],
		[5, 6],
		[11, 2],
		[13, 128],
	])
		assert.throws(() =>
			assertSaved(
				edit(b, (body) => {
					body[at + offset] = value;
				}),
				{ mask: 'selection' },
			),
		);
});
