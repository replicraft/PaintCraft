// Two canonical browser regressions, not a general PCND parser or copy authority.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';

const catalog = new URL(
	'../../src/paintcraft-render-worker/tests/source8-service-fixtures/oracle.json',
	import.meta.url,
);
const expectations = Object.freeze({
	'general-multiply-alpha': Object.freeze({
		sha256: '2f74fe76ddcf0cbf24e0d7061025c2ce858401f45aa06f978c4716a28feedb00',
		rgba: Object.freeze([86, 81, 89, 192, 86, 81, 89, 192]),
	}),
	'prepared-chain-0': Object.freeze({
		sha256: '0d077eb1ca597c71e51380aadf4ef975ca6efbd2a37569c546873cd770fbe5eb',
		rgba: Object.freeze([28, 116, 220, 255, 28, 116, 220, 255]),
	}),
});
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
const hex = (value, bytes) =>
	typeof value === 'string' &&
	value.length === bytes * 2 &&
	/^[0-9a-f]+$/.test(value);

function selected(id) {
	assert.equal(typeof id, 'string', 'known canonical fixture ID required');
	assert(Object.hasOwn(expectations, id), 'unknown canonical fixture ID');
	assert(
		fs.statSync(catalog).size <= 65536,
		'bounded tracked fixture catalog',
	);
	const input = fs.readFileSync(catalog);
	assert(input.length > 0 && input.length <= 65536);
	const parsed = JSON.parse(input);
	assert(parsed && typeof parsed === 'object' && Array.isArray(parsed.cases));
	assert(parsed.cases.length > 0 && parsed.cases.length <= 64);
	const matches = parsed.cases.filter((row) => row?.name === id);
	assert.equal(matches.length, 1, 'exactly one tracked selected fixture');
	const row = matches[0];
	assert(row && typeof row === 'object' && !Array.isArray(row));
	assert(hex(row.sha256, 32) && row.sha256 === expectations[id].sha256);
	assert(
		typeof row.hex === 'string' &&
			row.hex.length >= 86 &&
			row.hex.length <= 1024 &&
			row.hex.length % 2 === 0 &&
			/^[0-9a-f]+$/.test(row.hex),
		'bounded canonical source bytes',
	);
	const bytes = Buffer.from(row.hex, 'hex');
	assert.equal(sha(bytes), expectations[id].sha256);
	assert.equal(bytes.subarray(0, 8).toString('hex'), '50434e44080d0a1a');
	assert.equal(
		sha(bytes.subarray(0, -32)),
		bytes.subarray(-32).toString('hex'),
	);
	assert.deepEqual(row.canvas, [2, 1]);
	assert.deepEqual(row.visible, [0, 0, 2, 1]);
	assert.equal(row.level, 0);
	assert.equal(row.mode, 'Base');
	assert.equal(row.stored_noise, false);
	assert.equal(row.document, '10'.repeat(16));
	assert.equal(row.source_state, '11'.repeat(16));
	const rgba = Buffer.from(expectations[id].rgba);
	assert.equal(row.rgba_hex, rgba.toString('hex'));
	assert.equal(row.rgba_sha256, sha(rgba));
	return { bytes, sha256: expectations[id].sha256, rgba, canvas: [2, 1] };
}

/** Fresh owned source and independent two-pixel golden; only two IDs are accepted. */
export function fixture(id) {
	return selected(id);
}

/** Exact canonical source preservation only; compression-equivalent saves may differ. */
export function assertSaved(bytes, id) {
	assert(
		bytes instanceof Uint8Array,
		'saved bytes must be an owned byte view',
	);
	const expected = selected(id).bytes;
	assert.equal(
		bytes.byteLength,
		expected.length,
		'canonical saved source length',
	);
	assert.deepEqual(
		Buffer.from(bytes),
		expected,
		'canonical saved source changed',
	);
}
