import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import test from 'node:test';
import { boxPlane, spatialMix } from './lib/studio-box-oracle.mjs';
import { evaluated, project, sha } from './lib/studio-filter-oracle.mjs';

const numericFolder = new URL(
	'../../../crates/services/paintcraft-effects/tests/box-fixtures/',
	import.meta.url,
);
const numeric = JSON.parse(
	fs.readFileSync(new URL('oracle.json', numericFolder)),
);
const masks = [0, 1, 64, 85, 127, 128, 170, 254, 255];
test('independent full-square Box and one-blend mask literals including true source ROI partitions', () => {
	for (const row of numeric.literals) {
		assert.deepEqual(
			[
				...boxPlane(
					row.rgba,
					...row.size,
					row.radius,
					row.region,
					row.origin,
				),
			],
			row.expected,
			row.name,
		);
		for (const part of row.partitions)
			assert.deepEqual(
				[
					...boxPlane(
						row.rgba,
						...row.size,
						row.radius,
						part.region,
						row.origin,
					),
				],
				part.expected,
				row.name,
			);
	}
	for (const row of numeric.mask_literals)
		assert.deepEqual(
			spatialMix(row.incoming, row.candidate, row.coverage),
			row.expected,
		);
	for (const row of numeric.masked_planes) {
		const candidate = boxPlane(row.rgba, ...row.size, row.radius);
		assert.deepEqual([...candidate], row.rounded_candidate);
		const result = [];
		for (let p = 0; p < candidate.length; p += 4)
			result.push(
				...spatialMix(
					row.rgba.slice(p, p + 4),
					candidate.subarray(p, p + 4),
					row.coverage,
				),
			);
		assert.deepEqual(result, row.expected);
	}
});
test('54726 independent Fraction records bind three framed BigInt output stream hashes', () => {
	const single = createHash('sha256'),
		edges = createHash('sha256'),
		blend = createHash('sha256');
	let n = 0;
	for (let radius = 0; radius <= 32; radius++) {
		for (let alpha = 0; alpha <= 255; alpha++) {
			const input = [17, 93, 201, alpha];
			single.update(
				Buffer.from([radius, alpha, ...boxPlane(input, 1, 1, radius)]),
			);
			n++;
		}
		for (
			let pair = 0;
			pair < numeric.source_recipe.edge_pairs.length;
			pair++
		) {
			const output = boxPlane(
				numeric.source_recipe.edge_pairs[pair],
				2,
				1,
				radius,
			);
			for (let x = 0; x < 2; x++) {
				edges.update(
					Buffer.from([
						radius,
						pair,
						x,
						...output.subarray(x * 4, x * 4 + 4),
					]),
				);
				n++;
			}
		}
	}
	for (
		let pair = 0;
		pair < numeric.source_recipe.mask_rgb_pairs.length;
		pair++
	) {
		const [p, q] = numeric.source_recipe.mask_rgb_pairs[pair];
		for (let alpha = 0; alpha <= 255; alpha++)
			for (const other of [0, 1, 127, 128, 255])
				for (const m of masks) {
					blend.update(
						Buffer.from([
							pair,
							alpha,
							other,
							m,
							...spatialMix([...p, alpha], [...q, other], m),
						]),
					);
					n++;
				}
	}
	assert.equal(n, 54726);
	assert.equal(
		single.digest('hex'),
		numeric.streams.one_by_one_all_alphas_all_radii.sha256,
	);
	assert.equal(
		edges.digest('hex'),
		numeric.streams.two_pixel_edges_all_radii.sha256,
	);
	assert.equal(
		blend.digest('hex'),
		numeric.streams.masked_all_input_alphas_selected_candidate_alphas
			.sha256,
	);
});
const folder = new URL(
	'../../../crates/services/paintcraft-formats/tests/pcnd-spatial-fixtures/',
	import.meta.url,
);
const oracle = JSON.parse(fs.readFileSync(new URL('oracle.json', folder)));
const descriptors = {
	1: 'invert-rgb-v1',
	2: 'brightness-contrast-v1',
	3: 'black-white-rgb-v1',
	4: 'threshold-luma-v1',
	5: 'posterize-rgb-v1',
	6: 'solarize-rgb-v1',
	7: 'box-blur-v1',
};
for (const row of oracle.fixtures)
	test(`independent PCND5 closed registry and all-entry masks: ${row.file}`, () => {
		const bytes = fs.readFileSync(new URL(row.file, folder));
		assert.equal(sha(bytes), row.sha256);
		if (!row.accepted) {
			assert.throws(() => project(bytes));
			return;
		}
		const doc = project(bytes),
			layer = doc.layers[0];
		assert.equal(doc.version, 5);
		assert.equal(doc.document, oracle.document);
		assert.equal(doc.state, oracle.state);
		assert.deepEqual(layer.origin, row.origin);
		assert.equal(layer.rgba, Buffer.from(row.raw_rgba).toString('hex'));
		assert.equal(layer.filters.length, row.filters.length);
		for (let i = 0; i < row.filters.length; i++) {
			const actual = layer.filters[i],
				expected = row.filters[i];
			assert.equal(actual.id, expected.id);
			assert.equal(Boolean(actual.enabled), expected.enabled);
			const { kind: tag, id, enabled, mask, ...params } = actual;
			assert.deepEqual(
				{ kind: descriptors[tag], ...params },
				expected.descriptor,
			);
			if (expected.mask) {
				assert.equal(Boolean(mask.enabled), expected.mask.enabled);
				assert.deepEqual(mask.domain, expected.mask.domain);
				assert.deepEqual(mask.bands, expected.mask.bands);
			} else assert.equal(mask, null);
		}
		if (row.attachment) {
			assert.equal(doc.attachment.header, oracle.source_header_hex);
			assert.equal(doc.attachment.profileSha, oracle.profile_sha256);
		} else assert.equal(doc.attachment, undefined);
	});
test('UI default2 and changed1 independently produce distinct causal red/green pairs', () => {
	const doc = {
		version: 5,
		width: 2,
		height: 1,
		layers: [
			{
				origin: [0, 0],
				visible: 1,
				opacity: 255,
				width: 2,
				height: 1,
				rgba: 'ff0000ff00c800ff',
				filters: [],
			},
		],
	};
	for (const [radius, expected] of [
		[2, [153, 80, 0, 255, 102, 120, 0, 255]],
		[1, [170, 67, 0, 255, 85, 133, 0, 255]],
	]) {
		doc.layers[0].filters = [{ kind: 7, radius, enabled: 1 }];
		assert.deepEqual([...evaluated(doc)], expected);
	}
});

test('independent source-inventory framing matches all 23 frozen point identities and native default source', async () => {
	const { inventory, nativeSource, spatialPlan } = await import(
		'./lib/spatial-worker-fixtures.mjs'
	);
	const pointFolder = new URL(
		'../../../crates/services/paintcraft-formats/tests/pcnd-point-fixtures/',
		import.meta.url,
	);
	const points = JSON.parse(
		fs.readFileSync(new URL('oracle.json', pointFolder)),
	);
	let count = 0;
	for (const row of points.fixtures.filter((r) => r.accepted)) {
		const source = fs.readFileSync(new URL(row.file, pointFolder)),
			doc = project(source);
		assert.equal(
			inventory(doc, row.stack_sha256),
			row.filter_inventory_sha256,
		);
		count++;
	}
	assert.equal(count, 23);
	const native = nativeSource(oracle),
		doc = project(native.source),
		plan = spatialPlan(native.source, doc, native.stack, 0);
	assert.equal(doc.version, 1);
	assert.equal(plan.spatialCount, 0);
	assert.equal(plan.residentBytes, 0);
	assert.deepEqual(plan.size, [256, 256]);
	assert.deepEqual(doc.layers[0].default, [20, 40, 60, 255]);
	assert.equal(doc.layers[0].rgba, '14283cff'.repeat(256 * 256));
});
