// Cross-language bound checks: no native decoder or browser dependency.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { LINEAR_MEMORY_BYTES } from '../src/lib/codec-worker-protocol.js';
import {
	freezeMspPlan,
	MSP_ENCODED_MAX,
	MSP_OUTPUT_MAX,
	MSP_PREVIEW_MAX,
	MSP_REQUEST_HEADER,
	MSP_RESERVATION,
	MSP_RESPONSE_HEADER,
	MSP_SOURCE_MAX,
} from '../src/lib/msp-worker-protocol.js';
import { context } from './test-support/source-metadata-context.mjs';

const source = fs.readFileSync(
	new URL(
		'../../../crates/services/paintcraft-formats/src/msp/web_export.rs',
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
	return expression.split('+').reduce((sum, term) => {
		const part = /^(\d+)(?:<<(\d+))?$/.exec(term);
		assert.ok(part, `Finite literal/shift/addition required: ${name}`);
		return sum + Number(part[1]) * 2 ** Number(part[2] ?? 0);
	}, 0);
}
test('MSP Rust and JS request/output/admission caps agree exactly', () => {
	assert.equal(MSP_SOURCE_MAX, rustNumber('SOURCE_BYTES_MAX'));
	assert.equal(MSP_ENCODED_MAX, rustNumber('ENCODED_BYTES_MAX'));
	assert.equal(MSP_PREVIEW_MAX, rustNumber('PREVIEW_SIDE') ** 2 * 4);
	assert.equal(MSP_REQUEST_HEADER, rustNumber('REQUEST_HEADER'));
	assert.equal(MSP_RESPONSE_HEADER, rustNumber('RESPONSE_HEADER'));
	assert.equal(MSP_RESERVATION, rustNumber('RESERVATION_BYTES'));
	assert.equal(
		MSP_OUTPUT_MAX,
		MSP_RESPONSE_HEADER + MSP_ENCODED_MAX + MSP_PREVIEW_MAX + 4096,
	);
	assert.equal(rustNumber('PIXELS_MAX'), 2048 * 2048);
	const plan = (width, height) => ({
		width,
		height,
		sourceFlags: 0,
		metadataContext: context(2),
		version: 2,
	});
	assert.doesNotThrow(() => freezeMspPlan(plan(2048, 2048)));
	assert.doesNotThrow(() => freezeMspPlan(plan(65535, 64)));
	assert.throws(() => freezeMspPlan(plan(2049, 2048)));
	assert.throws(() => freezeMspPlan(plan(65535, 65)));
	assert.throws(() => freezeMspPlan(plan(65536, 1)));
});
test('the logical declaration covers conservative payload slots without an RSS claim', () => {
	const quote =
		LINEAR_MEMORY_BYTES +
		5 * MSP_SOURCE_MAX +
		5 * MSP_ENCODED_MAX +
		rustNumber('RESIDENT_BYTES_MAX') +
		8 * MSP_PREVIEW_MAX +
		1024 * 1024;
	assert.ok(quote <= MSP_RESERVATION);
	assert.equal(rustNumber('LAYERS_MAX'), 8);
	assert.equal(rustNumber('STAGED_LAYER_PIXELS_MAX'), 8 * 1024 * 1024);
});
