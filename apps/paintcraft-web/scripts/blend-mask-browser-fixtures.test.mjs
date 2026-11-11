import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import test from 'node:test';
import { fixture, assertSaved } from './lib/blend-mask-browser-fixtures.mjs';
const vectors = JSON.parse(
	fs.readFileSync(
		new URL('./fixtures/blend-mask-saved-vectors.json', import.meta.url),
	),
).cases;
const hash = (b) => createHash('sha256').update(b).digest();
const authenticate = (body) => Buffer.concat([body, hash(body)]);
function mutate(bytes, edit) {
	const body = Buffer.from(bytes.subarray(0, -32));
	edit(body);
	return authenticate(body);
}
test('literal PCND8 mask states preserve both raw layers and independent full RGBA', () => {
	const base = fixture();
	for (const v of vectors) {
		const bytes = Buffer.from(v.hex, 'hex');
		assert.equal(hash(bytes).toString('hex'), v.sha256);
		const got = assertSaved(bytes, { mask: v.mask });
		assert.deepEqual([...got.fullRGBA], v.rgba);
		assert.deepEqual(got.rawIdentity, base.rawIdentity);
		assert.deepEqual(got.mask?.coverage ?? null, v.coverage);
		assert.notEqual(got.state, base.state);
	}
});
test('fresh ownership, offset byte views and nonnil state remain independent', () => {
	const a = fixture(),
		b = fixture();
	a.bytes.fill(0);
	a.rgba.fill(0);
	a.layers[0].default[0] = 0;
	assert.deepEqual(fixture(), b);
	const v = vectors.find((v) => v.mask === 'right');
	const bytes = Buffer.from(v.hex, 'hex'),
		padded = Buffer.concat([Buffer.from([9]), bytes, Buffer.from([8])]);
	const got = assertSaved(
		new Uint8Array(padded.buffer, padded.byteOffset + 1, bytes.length),
		{ mask: 'right' },
	);
	got.mask.coverage.fill(0);
	got.layers[0].default.fill(0);
	assert.deepEqual([...assertSaved(bytes, { mask: 'right' }).rgba], v.rgba);
});
test('authenticated raw/source mutations, nil states and malformed framing refuse', () => {
	const source = fixture().bytes;
	for (const edit of [
		(b) => b.fill(0, 26, 42),
		(b) => {
			b[0] |= 128;
		},
		(b) => {
			b[4] = 7;
		},
		(b) => {
			b[10] ^= 1;
		},
		(b) => {
			b[b.indexOf(Buffer.from([31, 127, 241, 128]))] ^= 1;
		},
		(b) => {
			b[b.indexOf(Buffer.from([204, 77, 13, 128]))] ^= 1;
		},
	])
		assert.throws(() =>
			assertSaved(mutate(source, edit), { mask: 'none' }),
		);
	for (const bad of [
		source.subarray(1),
		Buffer.concat([source, Buffer.from([0])]),
		new Uint8Array(1025),
		[],
	])
		assert.throws(() => assertSaved(bad, { mask: 'none' }));
	assert.throws(() => assertSaved(source, { mask: 'right' }));
	assert.throws(() => assertSaved(source, { mask: 'other' }));
});
test('wrong mask side/enabled/domain/codec/coverage is refused despite valid authentication', () => {
	const bytes = Buffer.from(
		vectors.find((v) => v.mask === 'right').hex,
		'hex',
	);
	assert.throws(() => assertSaved(bytes, { mask: 'left' }));
	assert.throws(() => assertSaved(bytes, { mask: 'disabled' }));
	const needle = Buffer.from([1, 1, 1, 0, 0, 4, 2, 1, 0, 1, 1, 1, 1, 255]);
	const at = bytes.indexOf(needle);
	assert(at >= 0);
	for (const [offset, value] of [
		[2, 2],
		[5, 2],
		[13, 128],
		[8, 1],
		[11, 2],
	])
		assert.throws(() =>
			assertSaved(
				mutate(bytes, (body) => {
					body[at + offset] = value;
				}),
				{ mask: 'right' },
			),
		);
});
