import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import test from 'node:test';
import {
	fixture,
	inspectSaved,
	assertSaved,
} from './lib/layer-mask-browser-fixtures.mjs';

const spec = JSON.parse(
	fs.readFileSync(
		new URL(
			'./fixtures/studio-layer-mask-saved-vectors.json',
			import.meta.url,
		),
	),
);
const vector = (mask, invert = false) =>
	spec.cases.find((v) => v.mask === mask && v.invert === invert);
const bytes = (v) => Buffer.from(v.hex, 'hex');
function authenticate(body) {
	return Buffer.concat([body, createHash('sha256').update(body).digest()]);
}
function change(value, edit) {
	const body = Buffer.from(value.subarray(0, -32));
	edit(body);
	return authenticate(body);
}
function replace(value, before, after) {
	const body = value.subarray(0, -32),
		needle = Buffer.from(before, 'hex');
	const at = body.indexOf(needle);
	assert(at >= 0 && body.indexOf(needle, at + 1) === -1);
	return authenticate(
		Buffer.concat([
			body.subarray(0, at),
			Buffer.from(after, 'hex'),
			body.subarray(at + needle.length),
		]),
	);
}

test('literal PCND1/7 specimens preserve raw identity and independent full mask pixels', () => {
	const original = fixture();
	assert.equal(original.sha256, spec.fixture_sha256);
	const identity = inspectSaved(original.bytes).rawIdentity;
	assert.deepEqual([...original.rgba], spec.raw_rgba);
	for (const v of spec.cases) {
		const saved = bytes(v),
			decoded = assertSaved(saved, { mask: v.mask, invert: v.invert });
		assert.equal(
			createHash('sha256').update(saved).digest('hex'),
			v.sha256,
		);
		assert.deepEqual(decoded.rawIdentity, identity);
		assert.deepEqual([...decoded.rawRGBA], spec.raw_rgba);
		assert.deepEqual([...decoded.rgba], v.rgba);
		assert.deepEqual(decoded.mask?.coverage ?? null, v.coverage);
		assert.equal(
			decoded.mask?.enabled ?? null,
			v.mask === 'none' ? null : v.mask !== 'disabled',
		);
		assert.equal(decoded.version, v.mask === 'none' ? 1 : 7);
	}
});

test('caller mutations do not escape and state/filter UUIDs remain inspectable', () => {
	const good = fixture(),
		damaged = fixture();
	damaged.bytes.fill(0);
	damaged.rgba.fill(0);
	damaged.canvas[0] = 9;
	assert.deepEqual(fixture(), good);
	const selected = bytes(vector('selection')),
		before = inspectSaved(selected);
	const edited = change(selected, (body) => body.fill(0xb5, 26, 42));
	// PCND7 empty catalog prefix adds two bytes before the fixed document/state IDs.
	const after = assertSaved(edited, { mask: 'selection', invert: false });
	assert.notEqual(after.state, before.state);
	assert.deepEqual(after.rawIdentity, before.rawIdentity);
	after.mask.coverage.fill(0);
	after.rawIdentity.canvas[0] = 999;
	after.rawRGBA.fill(0);
	assert.deepEqual(inspectSaved(selected), before);
	const filtered = bytes(vector('selection', true));
	const changedId = replace(filtered, spec.ids.filter, 'b6'.repeat(16));
	assert.notEqual(
		assertSaved(changedId, { mask: 'selection', invert: true }).filters[0]
			.id,
		inspectSaved(filtered).filters[0].id,
	);
	const padded = Buffer.concat([
		Buffer.from([8]),
		good.bytes,
		Buffer.from([9]),
	]);
	assertSaved(
		new Uint8Array(padded.buffer, padded.byteOffset + 1, good.bytes.length),
		{ mask: 'none', invert: false },
	);
});

test('wrong requested mask/filter states and malformed authenticated framing refuse', () => {
	const original = fixture().bytes,
		selected = bytes(vector('selection'));
	for (const bad of [
		original.subarray(0, -1),
		Buffer.concat([original, Buffer.from([0])]),
		new Uint8Array(4097),
		[],
	]) {
		assert.throws(() => inspectSaved(bad));
	}
	const corrupt = Buffer.from(selected);
	corrupt[50] ^= 1;
	assert.throws(() => inspectSaved(corrupt));
	for (const edit of [
		(b) => (b[0] |= 128),
		(b) => (b[4] = 8),
		(b) => (b[8] ^= 1),
		(b) => (b[b.length - 1] = 1),
	]) {
		assert.throws(() => inspectSaved(change(original, edit)));
	}
	const body = original.subarray(0, -32);
	assert.throws(() =>
		inspectSaved(
			authenticate(
				Buffer.concat([
					body.subarray(0, 40),
					Buffer.from([0x84, 0]),
					body.subarray(41),
				]),
			),
		),
	);
	assert.throws(() =>
		inspectSaved(Buffer.from(spec.negative_v7_no_mask, 'hex')),
	);
	assert.throws(() =>
		assertSaved(selected, { mask: 'inverse', invert: false }),
	);
	assert.throws(() =>
		assertSaved(selected, { mask: 'selection', invert: true }),
	);
	assert.throws(() =>
		assertSaved(selected, {
			mask: 'selection',
			invert: false,
			extra: true,
		}),
	);
	assert.throws(() => assertSaved(selected, { mask: 'none', invert: true }));
	assert.throws(() =>
		assertSaved(selected, { mask: 'empty', invert: false }),
	);
});

test('A8 domain/codec/ordering and the single allowed descriptor are strict', () => {
	const selected = bytes(vector('selection'));
	const needle = '01010100000804010001010102ff';
	for (const invalid of [
		'01010200000804010001010102ff', // unsupported A8 codec
		'01010100000a04010001010102ff', // escaped fixed layer domain
		'01010100000804010001020101ff0201ff', // adjacent equal runs must merge
		'01010100000804010001020301ff0101ff', // out-of-order runs
		'0101010000080401000101010200', // explicit zero coverage is noncanonical
	])
		assert.throws(() => inspectSaved(replace(selected, needle, invalid)));
	const filtered = bytes(vector('selection', true));
	const unsupported = replace(
		filtered,
		Buffer.from('invert-rgb-v1').toString('hex'),
		Buffer.from('invent-rgb-v1').toString('hex'),
	);
	assert.throws(() => inspectSaved(unsupported));
});
