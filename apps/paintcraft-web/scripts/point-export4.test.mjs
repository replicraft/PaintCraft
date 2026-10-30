import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { pointExport4Request, pointExportFiles } from './lib/point-export4.mjs';
import { sha } from './lib/studio-filter-oracle.mjs';

const folder = new URL(
	'../../../crates/services/paintcraft-formats/tests/pcnd-point-fixtures/',
	import.meta.url,
);
const oracle = JSON.parse(fs.readFileSync(new URL('oracle.json', folder)));
test('point Export4 closed corpus encodes explicit independent bake, mask and source authorities', () => {
	assert.equal(pointExportFiles.length, 8);
	let cases = 0;
	for (const name of pointExportFiles) {
		const row = oracle.fixtures.find((entry) => entry.file === name),
			source = fs.readFileSync(new URL(name, folder));
		assert.equal(sha(source), row.sha256);
		assert.equal(row.attachment, false);
		for (const kind of ['dds', 'msp'])
			for (const bake of [false, true]) {
				const bytes = pointExport4Request(
						source,
						row,
						oracle,
						kind,
						bake,
					),
					view = new DataView(bytes.buffer);
				assert.equal(
					Buffer.from(bytes.subarray(0, 8)).toString(),
					kind === 'dds' ? 'PCDDSQ04' : 'PCMSPQ04',
				);
				assert.equal(view.getUint16(8), 4);
				assert.equal(bytes.length, 256 + source.length);
				assert.equal(view.getUint32(12), source.length);
				assert.deepEqual(
					[view.getUint32(16), view.getUint32(20)],
					[2, 1],
				);
				assert.equal(
					Buffer.from(bytes.subarray(81, 113)).toString('hex'),
					row.sha256,
				);
				assert.equal(
					Buffer.from(bytes.subarray(128, 160)).toString('hex'),
					row.filter_inventory_sha256,
				);
				assert.equal(bytes[122], Number(bake));
				assert.equal(
					bytes[kind === 'dds' ? 121 : 114],
					row.mask_count ? 12 : 4,
				);
				assert.deepEqual(
					[...bytes.subarray(176, 209)],
					Array(33).fill(0),
					'no metadata provenance or omission authority',
				);
				assert.deepEqual(
					[bytes[209], bytes[210], bytes[211]],
					[kind === 'dds' ? 1 : 2, 1, 0],
				);
				assert.deepEqual(
					[224, 232, 240, 248].map((at) =>
						Number(view.getBigUint64(at, true)),
					),
					[
						row.mask_count,
						row.bands,
						row.runs,
						row.bands ? 2097152 : 0,
					],
				);
				assert.deepEqual(Buffer.from(bytes.subarray(256)), source);
				cases++;
			}
	}
	assert.equal(cases, 32);
	const row = oracle.fixtures.find(
		(entry) => entry.file === 'source-all-six.pcnd',
	);
	assert.throws(
		() => pointExport4Request(new Uint8Array(), row, oracle, 'dds', true),
		/Closed/,
	);
});
