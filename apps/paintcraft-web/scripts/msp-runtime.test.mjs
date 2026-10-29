// Execute the actual runtime source with only its Wasm imports substituted.
// This exercises dispatch/copy caps, not real Wasm or browser qualification.

import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';
import {
	INPUT_BYTES_MAX,
	LINEAR_MEMORY_BYTES,
	OUTPUT_BYTES_MAX,
	REQUEST_HEADER,
} from '../src/lib/codec-worker-protocol.js';
import {
	DDS_OUTPUT_MAX,
	DDS_REQUEST_HEADER,
	DDS_SOURCE_MAX,
} from '../src/lib/dds-worker-protocol.js';
import {
	MSP_OUTPUT_MAX,
	MSP_REQUEST_HEADER,
	MSP_SOURCE_MAX,
} from '../src/lib/msp-worker-protocol.js';
import {
	TIFF_OUTPUT_MAX,
	TIFF_PROTOCOL_VERSION,
	TIFF_REQUEST_HEADER,
	TIFF_SOURCE_MAX,
} from '../src/lib/tiff-page-worker-protocol.js';

const source = fs
	.readFileSync(
		new URL('../src/lib/codec-worker-runtime.js', import.meta.url),
		'utf8',
	)
	.replace(/^import [\s\S]*? from '[^']+';\n/gm, '');
function runtime(extra = {}) {
	const calls = [],
		messages = [];
	const execute = (kind) => (bytes) => {
		calls.push({ kind, bytes });
		return new Uint8Array([1, 2, 3]);
	};
	const self = {
		postMessage(value, transfer) {
			messages.push({ value, transfer });
		},
	};
	vm.runInNewContext(source, {
		self,
		Uint8Array,
		ArrayBuffer,
		RangeError,
		initialize: async () => ({
			memory: { buffer: new ArrayBuffer(65536) },
		}),
		protocol_version: () => 2,
		linear_memory_ceiling: () => LINEAR_MEMORY_BYTES,
		application_version: () => '0.1.0',
		dds_export_protocol_version: () => 1,
		msp_export_protocol_version: () => 1,
		tiff_page_protocol_version: () => 2,
		decode_request: execute('decode'),
		dds_export_request: execute('dds-export'),
		msp_export_request: execute('msp-export'),
		tiff_page_request: execute('tiff-pages'),
		INPUT_BYTES_MAX,
		REQUEST_HEADER,
		OUTPUT_BYTES_MAX,
		LINEAR_MEMORY_BYTES,
		DDS_SOURCE_MAX,
		DDS_REQUEST_HEADER,
		DDS_OUTPUT_MAX,
		MSP_SOURCE_MAX,
		MSP_REQUEST_HEADER,
		MSP_OUTPUT_MAX,
		TIFF_REQUEST_HEADER,
		TIFF_SOURCE_MAX,
		TIFF_PROTOCOL_VERSION,
		TIFF_OUTPUT_MAX,
		...extra,
	});
	return { self, calls, messages };
}
test('one shared runtime selects only the exact operation and transfers one result', async () => {
	for (const [kind, header] of [
		['decode', REQUEST_HEADER],
		['dds-export', DDS_REQUEST_HEADER],
		['msp-export', MSP_REQUEST_HEADER],
		['tiff-pages', TIFF_REQUEST_HEADER],
	]) {
		const r = runtime();
		await r.self.onmessage({
			data: { kind, bytes: new ArrayBuffer(header + 1) },
		});
		assert.equal(r.calls.length, 1);
		assert.equal(r.calls[0].kind, kind);
		assert.equal(r.messages.length, 1);
		assert.equal(r.messages[0].value.kind, 'result');
		assert.equal(r.messages[0].transfer[0], r.messages[0].value.bytes);
		await r.self.onmessage({
			get data() {
				throw new Error('second message accessed');
			},
		});
		assert.equal(r.calls.length, 1);
		assert.equal(r.messages.length, 1);
	}
});
test('invalid operation, profile version, input and output caps cannot call another codec', async () => {
	for (const kind of [
		'constructor',
		'toString',
		'msp',
		'decode2',
		undefined,
	]) {
		const r = runtime();
		await r.self.onmessage({ data: { kind, bytes: new ArrayBuffer(128) } });
		assert.equal(r.calls.length, 0);
		assert.equal(r.messages[0].value.kind, 'failure');
	}
	for (const [kind, header, max] of [
		['decode', REQUEST_HEADER, INPUT_BYTES_MAX],
		['dds-export', DDS_REQUEST_HEADER, DDS_SOURCE_MAX],
		['msp-export', MSP_REQUEST_HEADER, MSP_SOURCE_MAX],
		['tiff-pages', TIFF_REQUEST_HEADER, TIFF_SOURCE_MAX],
	]) {
		for (const length of [header - 1, header + max + 1]) {
			const r = runtime();
			await r.self.onmessage({
				data: { kind, bytes: new ArrayBuffer(length) },
			});
			assert.equal(r.calls.length, 0);
			assert.equal(r.messages[0].value.kind, 'failure');
		}
	}
	const badVersion = runtime({ msp_export_protocol_version: () => 2 });
	await badVersion.self.onmessage({
		data: { kind: 'msp-export', bytes: new ArrayBuffer(129) },
	});
	assert.equal(badVersion.calls.length, 0);
	assert.equal(badVersion.messages[0].value.kind, 'failure');
	const badOutput = runtime({
		msp_export_request: () => new Uint8Array(MSP_OUTPUT_MAX + 1),
	});
	await badOutput.self.onmessage({
		data: { kind: 'msp-export', bytes: new ArrayBuffer(129) },
	});
	assert.equal(badOutput.messages[0].value.kind, 'failure');
	const tiffVersion = runtime({ tiff_page_protocol_version: () => 1 });
	await tiffVersion.self.onmessage({
		data: {
			kind: 'tiff-pages',
			bytes: new ArrayBuffer(TIFF_REQUEST_HEADER + 1),
		},
	});
	assert.equal(tiffVersion.calls.length, 0);
	assert.equal(tiffVersion.messages[0].value.kind, 'failure');
	const tiffOutput = runtime({
		tiff_page_request: () => new Uint8Array(TIFF_OUTPUT_MAX + 1),
	});
	await tiffOutput.self.onmessage({
		data: {
			kind: 'tiff-pages',
			bytes: new ArrayBuffer(TIFF_REQUEST_HEADER + 1),
		},
	});
	assert.equal(tiffOutput.messages[0].value.kind, 'failure');
});
