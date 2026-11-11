import assert from 'node:assert/strict';
import test from 'node:test';
import { assertSaved, fixture } from './lib/main-blend-browser-fixtures.mjs';

const ids = ['general-multiply-alpha', 'prepared-chain-0'];

test('callers cannot corrupt later source, golden pixels or canvas dimensions', () => {
	for (const id of ids) {
		const expected = fixture(id);
		const damaged = fixture(id);
		damaged.bytes.fill(0);
		damaged.rgba.fill(0);
		damaged.canvas[0] = 999;
		assert.deepEqual(fixture(id), expected);
		assertSaved(expected.bytes, id);
	}
});

test('canonical saved bytes reject mutation, truncation, suffix and wrong source', () => {
	for (const id of ids) {
		const { bytes } = fixture(id);
		const changed = Buffer.from(bytes);
		changed[Math.floor(changed.length / 2)] ^= 1;
		for (const invalid of [
			changed,
			bytes.subarray(0, -1),
			Buffer.concat([bytes, Buffer.from([0])]),
			fixture(ids.find((other) => other !== id)).bytes,
		]) {
			assert.throws(() => assertSaved(invalid, id));
		}
		// A real offset byte view compares only its selected extent.
		const padded = Buffer.concat([
			Buffer.from([3]),
			bytes,
			Buffer.from([4]),
		]);
		assertSaved(
			new Uint8Array(padded.buffer, padded.byteOffset + 1, bytes.length),
			id,
		);
	}
	for (const id of ['__proto__', 'constructor', 'visible-codec-raw', null]) {
		assert.throws(() => fixture(id));
		assert.throws(() => assertSaved(new Uint8Array(), id));
	}
	assert.throws(() => assertSaved([], ids[0]));
});
