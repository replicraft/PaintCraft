// Cross-language bound checks: no native decoder or browser dependency.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { LINEAR_MEMORY_BYTES } from '../src/lib/codec-worker-protocol.js';
import {
	DDS_ENCODED_MAX,
	DDS_OUTPUT_MAX,
	DDS_PREVIEW_MAX,
	DDS_REQUEST_HEADER,
	DDS_RESERVATION,
	DDS_RESPONSE_HEADER,
	DDS_SOURCE_MAX,
	freezeDdsPlan,
} from '../src/lib/dds-worker-protocol.js';
import { context } from './test-support/source-metadata-context.mjs';

const source = fs.readFileSync(
	new URL(
		'../../../crates/services/paintcraft-formats/src/dds/web_export.rs',
		import.meta.url,
	),
	'utf8',
);
function rustNumber(name) {
	const match = source.match(
		new RegExp(`pub const ${name}: (?:u64|usize|u32|u16) = ([^;]+);`),
	);
	assert.ok(match, name);
	const expression = match[1].replace(/[\s_()]/g, '');
	const parts = /^(\d+)(?:<<(\d+))?(?:\+(\d+))?$/.exec(expression);
	assert.ok(parts, `Finite literal/shift/addition required: ${name}`);
	return (
		Number(parts[1]) * 2 ** Number(parts[2] ?? 0) + Number(parts[3] ?? 0)
	);
}
test('DDS Rust and JS request/output/admission caps agree exactly', () => {
	assert.equal(DDS_SOURCE_MAX, rustNumber('SOURCE_BYTES_MAX'));
	assert.equal(DDS_ENCODED_MAX, rustNumber('ENCODED_BYTES_MAX'));
	assert.equal(DDS_PREVIEW_MAX, rustNumber('PREVIEW_SIDE') ** 2 * 4);
	assert.equal(DDS_REQUEST_HEADER, rustNumber('REQUEST_HEADER'));
	assert.equal(DDS_RESPONSE_HEADER, rustNumber('RESPONSE_HEADER'));
	assert.equal(DDS_RESERVATION, rustNumber('RESERVATION_BYTES'));
	assert.equal(
		DDS_OUTPUT_MAX,
		DDS_RESPONSE_HEADER + DDS_ENCODED_MAX + DDS_PREVIEW_MAX + 4096,
	);
	assert.equal(rustNumber('PIXELS_MAX'), 2048 * 2048);
	const plan = (width, height) => ({
		width,
		height,
		sourceFlags: 0,
		filterDigest: new Uint8Array(32).fill(17),
		bakeIntent: 0,
		metadataContext: context(1),
		settings: new Uint8Array([0, 0, 0, 1, 0, 0, 0, 0]),
	});
	assert.doesNotThrow(() => freezeDdsPlan(plan(2048, 2048)));
	assert.doesNotThrow(() => freezeDdsPlan(plan(65536, 64)));
	assert.throws(() => freezeDdsPlan(plan(2049, 2048)));
	assert.throws(() => freezeDdsPlan(plan(65536, 65)));
	assert.throws(() => freezeDdsPlan(plan(65537, 1)));
});
test('the logical declaration covers conservative payload slots without an RSS claim', () => {
	const quote =
		LINEAR_MEMORY_BYTES +
		5 * DDS_SOURCE_MAX +
		5 * DDS_ENCODED_MAX +
		rustNumber('RESIDENT_BYTES_MAX') +
		8 * DDS_PREVIEW_MAX +
		1024 * 1024;
	assert.ok(quote <= DDS_RESERVATION);
	assert.equal(rustNumber('LAYERS_MAX'), 8);
	assert.equal(rustNumber('STAGED_LAYER_PIXELS_MAX'), 8 * 1024 * 1024);
});
