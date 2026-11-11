import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import test from 'node:test';
import {
	fixture,
	assertSaved,
} from './lib/source8-filter-mask-browser-fixtures.mjs';
const hash = (b) => createHash('sha256').update(b).digest();
const vectors = JSON.parse(
	fs.readFileSync(
		new URL(
			'./fixtures/source8-filter-mask-saved-vectors.json',
			import.meta.url,
		),
	),
).cases;
const edit = (b, fn) => {
	const body = Buffer.from(b.subarray(0, -32));
	fn(body);
	return Buffer.concat([body, hash(body)]);
};
test('independent literal PCND8 bodies preserve two roots/Invert14 and full2RGBA', () => {
	const base = fixture();
	assert.deepEqual([...base.rgba], [29, 131, 237, 192, 29, 131, 237, 192]);
	for (const row of vectors) {
		const b = Buffer.from(row.hex, 'hex');
		assert.equal(hash(b).toString('hex'), row.sha256);
		const parsed = assertSaved(b, { mask: row.mask });
		assert.deepEqual([...parsed.fullRGBA], row.rgba);
		assert.deepEqual(parsed.rawIdentity, base.rawIdentity);
		assert.deepEqual(parsed.mask?.coverage ?? null, row.coverage);
		assert.notEqual(parsed.state, base.state);
	}
});
test('fresh ownership and offset byte views preserve later calls', () => {
	const a = fixture(),
		expected = fixture();
	a.bytes.fill(0);
	a.layers[1].filters[0].id = 'bad';
	a.fullRGBA.fill(0);
	assert.deepEqual(fixture(), expected);
	const row = vectors.find((v) => v.mask === 'right'),
		b = Buffer.from(row.hex, 'hex');
	const pad = Buffer.concat([Buffer.from([7]), b, Buffer.from([8])]);
	assert.deepEqual(
		assertSaved(new Uint8Array(pad.buffer, pad.byteOffset + 1, b.length), {
			mask: 'right',
		}),
		assertSaved(b, { mask: 'right' }),
	);
});
test('authenticated changed raw source/filter identity/descriptor and nil state refuse', () => {
	const b = fixture().bytes;
	for (const fn of [
		(v) => v.fill(0, 50, 66),
		(v) => {
			v[34] ^= 1;
		},
		(v) => {
			v[v.indexOf(Buffer.from('14'.repeat(16), 'hex'))] ^= 1;
		},
		(v) => {
			v[v.indexOf(Buffer.from('invert-rgb-v1'))] ^= 1;
		},
		(v) => {
			v[v.indexOf(Buffer.from([31, 127, 241, 128]))] ^= 1;
		},
		(v) => {
			v[v.indexOf(Buffer.from([204, 77, 13, 128]))] ^= 1;
		},
		(v) => {
			v[0] |= 128;
		},
	])
		assert.throws(() => assertSaved(edit(b, fn), { mask: 'none' }));
	for (const bad of [
		b.subarray(1),
		Buffer.concat([b, Buffer.from([0])]),
		new Uint8Array(1025),
		[],
	])
		assert.throws(() => assertSaved(bad, { mask: 'none' }));
});
test('mask side/codec/domain/coverage/enabled expectations remain strict', () => {
	const b = Buffer.from(vectors.find((v) => v.mask === 'right').hex, 'hex');
	assert.throws(() => assertSaved(b, { mask: 'none' }));
	assert.throws(() => assertSaved(b, { mask: 'disabled' }));
	const needle = Buffer.from([1, 1, 1, 0, 0, 4, 2, 1, 0, 1, 1, 1, 1, 255]),
		at = b.indexOf(needle);
	assert(at >= 0);
	for (const [offset, value] of [
		[2, 2],
		[5, 2],
		[11, 0],
		[13, 128],
	])
		assert.throws(() =>
			assertSaved(
				edit(b, (v) => {
					v[at + offset] = value;
				}),
				{ mask: 'right' },
			),
		);
});
