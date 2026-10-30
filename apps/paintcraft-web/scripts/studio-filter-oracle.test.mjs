import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { crc32, deflateSync } from 'node:zlib';
import {
	canvasMarker,
	evaluated,
	maskCoverage,
	project,
	rawSemantics,
	sha,
} from './lib/studio-filter-oracle.mjs';

test('RGB screenshot oracle locates literal color pairs and rejects wrong colors', async () => {
	const width = 320,
		height = 240;
	const raw = Buffer.alloc((width * 3 + 1) * height);
	for (let y = 180; y < 188; y++)
		for (let x = 180; x < 196; x++)
			raw.set(
				x < 188 ? [127, 128, 128] : [0, 255, 0],
				y * (width * 3 + 1) + 1 + x * 3,
			);
	const header = Buffer.alloc(13);
	header.writeUInt32BE(width, 0);
	header.writeUInt32BE(height, 4);
	header[8] = 8;
	header[9] = 2;
	const chunk = (name, data) => {
		const length = Buffer.alloc(4),
			check = Buffer.alloc(4);
		length.writeUInt32BE(data.length);
		const body = Buffer.concat([Buffer.from(name), data]);
		check.writeUInt32BE(crc32(body));
		return Buffer.concat([length, body, check]);
	};
	const png = Buffer.concat([
		Buffer.from('89504e470d0a1a0a', 'hex'),
		chunk('IHDR', header),
		chunk('IDAT', deflateSync(raw)),
		chunk('IEND', Buffer.alloc(0)),
	]);
	assert.deepEqual(
		await canvasMarker(png, [
			[127, 128, 128],
			[0, 255, 0],
		]),
		{ x: 180, y: 180, width: 16, height: 8, area: 128 },
	);
	await assert.rejects(
		canvasMarker(png, [
			[128, 128, 128],
			[0, 255, 0],
		]),
	);
	await assert.rejects(canvasMarker(png));
});

const folder = new URL(
	'../../../crates/services/paintcraft-formats/tests/pcnd-mask-fixtures/',
	import.meta.url,
);
const oracle = JSON.parse(fs.readFileSync(new URL('oracle.json', folder)));
for (const specimen of oracle.fixtures) {
	test(`independent PCND4 reader: ${specimen.file}`, () => {
		const bytes = fs.readFileSync(new URL(specimen.file, folder));
		assert.equal(sha(bytes), specimen.sha256);
		if (!specimen.accepted) {
			assert.throws(() => project(bytes));
			return;
		}
		const doc = project(bytes);
		assert.equal(doc.version, 4);
		const masks = doc.layers.flatMap((l) =>
			l.filters.map((f) => f.mask).filter(Boolean),
		);
		assert.equal(
			masks.reduce((n, m) => n + m.bands.length, 0),
			specimen.bands,
		);
		assert.equal(
			masks.reduce(
				(n, m) => n + m.bands.reduce((n, b) => n + b.runs.length, 0),
				0,
			),
			specimen.runs,
		);
		if (specimen.rendered_rgba) {
			assert.deepEqual([...evaluated(doc)], specimen.rendered_rgba);
			assert.deepEqual(
				[...Buffer.from(doc.layers[0].rgba, 'hex')],
				specimen.rgba,
			);
		}
	});
}

test('source-local coverage includes zero, disabled and outside-domain distinctions', () => {
	const doc = project(fs.readFileSync(new URL('bands.pcnd', folder)));
	const mask = doc.layers[0].filters[0].mask;
	assert.deepEqual(mask.domain, oracle.domain);
	for (const [x, y, value] of oracle.samples)
		assert.equal(maskCoverage(mask, x, y), value);
	assert.equal(maskCoverage(mask, -3, -1), 0);
	assert.equal(maskCoverage({ ...mask, enabled: 0 }, -3, -1), 255);
	assert.equal(maskCoverage(null, -3, -1), 255);
	const empty = project(fs.readFileSync(new URL('zero.pcnd', folder)));
	assert.equal(maskCoverage(empty.layers[0].filters[0].mask, 0, 0), 0);
});

test('ordered masks preserve hidden RGB and alpha without altering source semantics', () => {
	const doc = project(fs.readFileSync(new URL('bands.pcnd', folder)));
	const original = rawSemantics(doc);
	// Use the independent fixture's two source samples, then explicitly place
	// different fractional stages over the entire finite two-pixel source.
	const field = (coverage) => ({
		enabled: 1,
		domain: [0, 0, 2, 1],
		bands: [{ y0: 0, y1: 1, runs: [{ x0: 0, x1: 2, coverage }] }],
	});
	doc.layers[0].filters = [
		{ enabled: 1, kind: 1, mask: field(128) },
		{ enabled: 1, kind: 2, brightness: 20, contrast: 0, mask: field(127) },
	];
	// Half invert gives [128,128,127]; the next half-strength +51 gives
	// [153,153,152]. Each stage rounds before the next stage begins.
	assert.deepEqual(
		[...evaluated(doc)],
		[153, 153, 152, 0, 153, 153, 152, 255],
	);
	assert.deepEqual(rawSemantics(doc), original);
	doc.layers[0].filters.reverse();
	assert.deepEqual(
		[...evaluated(doc)],
		[128, 128, 127, 0, 128, 127, 127, 255],
	);
});

const pointFolder = new URL(
	'../../../crates/services/paintcraft-formats/tests/pcnd-point-fixtures/',
	import.meta.url,
);
const pointOracle = JSON.parse(
	fs.readFileSync(new URL('oracle.json', pointFolder)),
);
function descriptor(filter) {
	switch (filter.kind) {
		case 1:
			return { kind: 'invert-rgb-v1' };
		case 2:
			return {
				kind: 'brightness-contrast-v1',
				brightness: filter.brightness,
				contrast: filter.contrast,
			};
		case 3:
			return { kind: 'black-white-rgb-v1', strength: filter.strength };
		case 4:
			return { kind: 'threshold-luma-v1', level: filter.level };
		case 5:
			return { kind: 'posterize-rgb-v1', levels: filter.levels };
		case 6:
			return { kind: 'solarize-rgb-v1' };
		default:
			throw Error('Unknown descriptor');
	}
}
for (const specimen of pointOracle.fixtures) {
	test(`independent six-kind PCND reader and rational evaluator: ${specimen.file}`, () => {
		const bytes = fs.readFileSync(new URL(specimen.file, pointFolder));
		assert.equal(bytes.length, specimen.bytes);
		assert.equal(sha(bytes), specimen.sha256);
		if (!specimen.accepted) {
			assert.throws(() => project(bytes));
			return;
		}
		const doc = project(bytes),
			layer = doc.layers[0],
			before = structuredClone(doc);
		assert.equal(doc.version, specimen.version);
		assert.equal(Boolean(doc.attachment), specimen.attachment);
		assert.deepEqual(
			[...Buffer.from(layer.rgba, 'hex')],
			specimen.raw_rgba,
		);
		assert.equal(layer.filters.length, specimen.filters.length);
		for (let i = 0; i < layer.filters.length; i++) {
			const filter = layer.filters[i],
				expected = specimen.filters[i];
			assert.equal(filter.id, expected.id);
			assert.equal(Boolean(filter.enabled), expected.enabled);
			assert.deepEqual(descriptor(filter), expected.descriptor);
			assert.equal(Boolean(filter.mask), expected.coverage !== null);
			for (let y = 0; y < layer.height; y++)
				for (let x = 0; x < layer.width; x++)
					assert.equal(
						maskCoverage(filter.mask, x, y),
						expected.coverage === null
							? 255
							: x === 0
								? expected.coverage
								: 0,
					);
		}
		assert.deepEqual([...evaluated(doc)], specimen.evaluated_rgba);
		assert.deepEqual(
			doc,
			before,
			'Evaluation never rewrites source pixels, stack or attachment',
		);
	});
}

test('six-kind evaluator rejects unknown tags instead of interpreting them as contrast', () => {
	const doc = project(fs.readFileSync(new URL('all-six.pcnd', pointFolder)));
	for (const kind of [0, 7, 9, 255]) {
		doc.layers[0].filters = [
			{ kind, enabled: 1, brightness: 0, contrast: 0 },
		];
		assert.throws(() => evaluated(doc), /Unknown point-filter/);
	}
});

test('checksum-valid unknown wire tags 7 and 9 refuse even when disabled', () => {
	for (const name of ['unknown-enabled.pcnd', 'unknown-disabled.pcnd']) {
		const original = fs.readFileSync(new URL(name, pointFolder));
		const id = Buffer.from('00000000000000000000000000000001', 'hex');
		const offset = original.lastIndexOf(id) + 17;
		assert.ok(offset >= 17 && original[offset] === 7);
		for (const tag of [7, 9]) {
			const bytes = Buffer.from(original);
			bytes[offset] = tag;
			Buffer.from(sha(bytes.subarray(0, -32)), 'hex').copy(
				bytes,
				bytes.length - 32,
			);
			assert.throws(() => project(bytes));
		}
	}
});

const pointUiFolder = new URL(
	'../../../crates/services/paintcraft-formats/tests/browser-point-ui-fixtures/',
	import.meta.url,
);
const pointUiOracle = JSON.parse(
	fs.readFileSync(new URL('oracle.json', pointUiFolder)),
);
test('independent red-green UI source has exact raw identity-prefix semantics', () => {
	const specimen = pointUiOracle.fixtures[0],
		bytes = fs.readFileSync(new URL(specimen.file, pointUiFolder));
	assert.equal(sha(bytes), specimen.sha256);
	assert.equal(bytes.length, specimen.bytes);
	const doc = project(bytes);
	assert.equal(doc.version, 3);
	assert.equal(doc.document, pointUiOracle.document);
	assert.equal(doc.state, pointUiOracle.state);
	assert.equal(doc.layers[0].id, pointUiOracle.layer);
	assert.deepEqual([doc.width, doc.height], [2, 1]);
	assert.equal(doc.attachment, undefined);
	assert.deepEqual(doc.layers[0].filters, [
		{ id: specimen.filters[0].id, enabled: 1, kind: 3, strength: 0 },
	]);
	assert.deepEqual(
		[...Buffer.from(doc.layers[0].rgba, 'hex')],
		[255, 0, 0, 255, 0, 200, 0, 255],
	);
	assert.deepEqual([...evaluated(doc)], specimen.evaluated_rgba);
});
for (const row of pointUiOracle.cases) {
	test(`default ${row.kind} is independently causal and has two literal screenshot colors`, () => {
		const doc = project(
			fs.readFileSync(new URL('red-green.pcnd', pointUiFolder)),
		);
		const config = {
			bw: { kind: 3, field: 'strength' },
			threshold: { kind: 4, field: 'level' },
			posterize: { kind: 5, field: 'levels' },
			solarize: { kind: 6 },
		}[row.kind];
		const filter = {
			enabled: 1,
			kind: config.kind,
			...(config.field ? { [config.field]: row.default_parameter } : {}),
		};
		doc.layers[0].filters.push(filter);
		assert.deepEqual([...evaluated(doc)], row.default_rgba);
		assert.notDeepEqual(
			row.default_rgba,
			pointUiOracle.fixtures[0].raw_rgba,
		);
		assert.notDeepEqual(
			row.default_rgba.slice(0, 3),
			row.default_rgba.slice(4, 7),
		);
		if (config.field) filter[config.field] = row.changed_parameter;
		assert.deepEqual([...evaluated(doc)], row.changed_rgba);
	});
}
