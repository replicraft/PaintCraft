import * as ddsPrepared from '../src/lib/dds-prepared-worker-protocol.js';
import * as mspPrepared from '../src/lib/msp-prepared-worker-protocol.js';
import * as ddsSpatial from '../src/lib/dds-spatial-worker-protocol.js';
import * as mspSpatial from '../src/lib/msp-spatial-worker-protocol.js';
import * as ddsCatalog from '../src/lib/dds-catalog-worker-protocol.js';
import * as mspCatalog from '../src/lib/msp-catalog-worker-protocol.js';
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
	DDS_PROTOCOL_VERSION,
	DDS_REQUEST_HEADER,
	DDS_SOURCE_MAX,
} from '../src/lib/dds-worker-protocol.js';
import {
	MSP_OUTPUT_MAX,
	MSP_PROTOCOL_VERSION,
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
		ddsSpatial,
		mspSpatial,
		ddsCatalog,
		mspCatalog,
		ddsPrepared,
		mspPrepared,
		Uint8Array,
		ArrayBuffer,
		RangeError,
		initialize: async () => ({
			memory: { buffer: new ArrayBuffer(65536) },
		}),
		protocol_version: () => 2,
		linear_memory_ceiling: () => LINEAR_MEMORY_BYTES,
		application_version: () => '0.1.0',
		dds_export_protocol_version: () => DDS_PROTOCOL_VERSION,
		dds_spatial_export_protocol_version: () => 5,
		msp_spatial_export_protocol_version: () => 5,
		dds_spatial_export_request: execute('dds-spatial-export'),
		msp_spatial_export_request: execute('msp-spatial-export'),
		dds_catalog_export_protocol_version: () => 6,
		dds_prepared_export_protocol_version: () => 7,
		msp_prepared_export_protocol_version: () => 7,
		dds_prepared_export_request: execute('dds-prepared-export'),
		msp_prepared_export_request: execute('msp-prepared-export'),
		msp_catalog_export_protocol_version: () => 6,
		dds_catalog_export_request: execute('dds-catalog-export'),
		msp_catalog_export_request: execute('msp-catalog-export'),
		msp_export_protocol_version: () => MSP_PROTOCOL_VERSION,
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
		DDS_PROTOCOL_VERSION,
		MSP_SOURCE_MAX,
		MSP_REQUEST_HEADER,
		MSP_OUTPUT_MAX,
		MSP_PROTOCOL_VERSION,
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
		['dds-spatial-export', ddsSpatial.DDS_REQUEST_HEADER],
		['msp-spatial-export', mspSpatial.MSP_REQUEST_HEADER],
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
		[
			'dds-spatial-export',
			ddsSpatial.DDS_REQUEST_HEADER,
			ddsSpatial.DDS_SOURCE_MAX,
		],
		[
			'msp-spatial-export',
			mspSpatial.MSP_REQUEST_HEADER,
			mspSpatial.MSP_SOURCE_MAX,
		],
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
	const badVersion = runtime({ msp_export_protocol_version: () => 1 });
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

test('spatial5 has independent exact version getters and never falls back to4', async () => {
	for (const kind of ['dds', 'msp']) {
		const r = runtime({
			[`${kind}_spatial_export_protocol_version`]: () => 4,
		});
		await r.self.onmessage({
			data: {
				kind: `${kind}-spatial-export`,
				bytes: new ArrayBuffer(321),
			},
		});
		assert.equal(r.calls.length, 0);
		assert.equal(r.messages[0].value.kind, 'failure');
	}
});

test('catalog6 selects only its exact runtime and refuses older worker getters', async () => {
	for (const kind of ['dds', 'msp']) {
		const refused = runtime({
			[`${kind}_catalog_export_protocol_version`]: () => 5,
		});
		await refused.self.onmessage({
			data: {
				kind: `${kind}-catalog-export`,
				bytes: new ArrayBuffer(385),
			},
		});
		assert.equal(refused.calls.length, 0);
		assert.equal(refused.messages[0].value.kind, 'failure');
		const accepted = runtime();
		await accepted.self.onmessage({
			data: {
				kind: `${kind}-catalog-export`,
				bytes: new ArrayBuffer(385),
			},
		});
		assert.deepEqual(
			accepted.calls.map((call) => call.kind),
			[`${kind}-catalog-export`],
		);
		assert.equal(accepted.messages[0].value.kind, 'result');
	}
});

test('prepared7 uses only its named execution entry with exact getter, input and output bounds', async () => {
	for (const codec of ['dds', 'msp']) {
		const api = codec === 'dds' ? ddsPrepared : mspPrepared;
		const upper = codec.toUpperCase(),
			kind = `${codec}-prepared-export`;
		const valid = runtime();
		await valid.self.onmessage({
			data: { kind, bytes: new ArrayBuffer(449) },
		});
		assert.equal(valid.calls.length, 1);
		assert.equal(valid.calls[0].kind, kind);
		assert.equal(valid.messages[0].value.kind, 'result');
		const wrong = runtime({
			[`${codec}_prepared_export_protocol_version`]: () => 6,
		});
		await wrong.self.onmessage({
			data: { kind, bytes: new ArrayBuffer(449) },
		});
		assert.equal(wrong.calls.length, 0);
		assert.equal(wrong.messages[0].value.kind, 'failure');
		for (const length of [447, 449 + api[`${upper}_SOURCE_MAX`]]) {
			const denied = runtime();
			await denied.self.onmessage({
				data: { kind, bytes: new ArrayBuffer(length) },
			});
			assert.equal(denied.calls.length, 0);
			assert.equal(denied.messages[0].value.kind, 'failure');
		}
		const oversized = runtime({
			[`${codec}_prepared_export_request`]: () =>
				new Uint8Array(api[`${upper}_OUTPUT_MAX`] + 1),
		});
		await oversized.self.onmessage({
			data: { kind, bytes: new ArrayBuffer(449) },
		});
		assert.equal(oversized.messages[0].value.kind, 'failure');
	}
});
