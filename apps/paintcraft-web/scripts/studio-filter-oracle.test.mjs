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
