// Finite two-root Multiply+Invert filter-mask oracle; no product decoder import.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
const source = new URL(
	'../fixtures/source8-filter-mask-invert.pcnd',
	import.meta.url,
);
const SOURCE_SHA =
	'4ce1c5aa8f6d492e289b885deeff5e84fde81da66ea501df9a0c9b375e6d97db';

const sha = (b) => createHash('sha256').update(b).digest('hex');
const uuid = (s) =>
	s.replace(/^(.{8})(.{4})(.{4})(.{4})(.{12})$/, '$1-$2-$3-$4-$5');
const utf8 = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
const original = [29, 131, 237, 192];
const unfiltered = [86, 81, 89, 192];
const back = [31, 127, 241, 128];
const expectedLayers = [
	{
		id: uuid('12'.repeat(16)),
		name: 'Layer',
		visible: true,
		opacity: 255,
		blend: 0,
		origin: [0, 0],
		size: [2, 1],
		default: back,
		filters: [],
		tiles: [],
	},
	{
		id: uuid('13'.repeat(16)),
		name: 'Layer',
		visible: true,
		opacity: 255,
		blend: 1,
		origin: [0, 0],
		size: [2, 1],
		default: [204, 77, 13, 128],
		filters: [],
		tiles: [],
	},
];
class Reader {
	constructor(bytes) {
		this.bytes = bytes;
		this.pos = 0;
	}
	take(n) {
		assert(
			Number.isSafeInteger(n) &&
				n >= 0 &&
				this.pos + n <= this.bytes.length,
		);
		const b = this.bytes.subarray(this.pos, this.pos + n);
		this.pos += n;
		return b;
	}
	byte() {
		return this.take(1)[0];
	}
	u(max) {
		let n = 0;
		for (let i = 0; i < 2; i++) {
			const b = this.byte();
			n += (b & 127) * 2 ** (7 * i);
			if (b < 128) {
				assert((i === 0 || b !== 0) && n <= max);
				return n;
			}
		}
		throw Error('finite canonical integer');
	}
	z() {
		const n = this.u(4);
		return n % 2 ? -(n + 1) / 2 : n / 2;
	}
	flag() {
		const n = this.byte();
		assert(n <= 1);
		return Boolean(n);
	}
	id() {
		return uuid(this.take(16).toString('hex'));
	}
	text() {
		return utf8.decode(this.take(this.u(32)));
	}
}
function ownMask(r) {
	if (!r.flag()) return null;
	const enabled = r.flag();
	assert.equal(r.byte(), 1);
	const domain = [r.z(), r.z(), r.z(), r.z()];
	assert.deepEqual(domain, [0, 0, 2, 1]);
	// This fixture only admits exactly one opaque single-pixel band/run.
	assert.equal(r.u(1), 1);
	assert.equal(r.u(0), 0);
	assert.equal(r.u(1), 1);
	assert.equal(r.u(1), 1);
	const x0 = r.u(1);
	assert.equal(r.u(1), 1);
	assert.equal(r.byte(), 255);
	const coverage = x0 === 0 ? [255, 0] : [0, 255];
	return { enabled, domain, coverage };
}
function inspect(value) {
	assert(
		value instanceof Uint8Array &&
			value.byteLength >= 43 &&
			value.byteLength <= 1024,
		'finite extent before copy',
	);
	const bytes = Buffer.from(value);
	assert.equal(bytes.subarray(0, 8).toString('hex'), '50434e44080d0a1a');
	assert.equal(
		sha(bytes.subarray(0, -32)),
		bytes.subarray(-32).toString('hex'),
	);
	const r = new Reader(bytes.subarray(8, -32));
	assert.equal(r.u(25), 25);
	assert.equal(r.u(1), 1);
	assert.equal(r.text(), 'invert-rgb-v1');
	assert.equal(r.text(), 'params-v1');
	const document = r.id(),
		state = r.id();
	assert.equal(document, uuid('10'.repeat(16)));
	assert.notEqual(state, uuid('00'.repeat(16)));
	assert.deepEqual([r.u(2), r.u(1)], [2, 1]);
	assert.equal(r.flag(), false, 'no title');
	assert.equal(r.flag(), false, 'no source attachment');
	assert.equal(r.u(2), 2);
	const layers = [];
	for (let i = 0; i < 2; i++) {
		const layer = {
			id: r.id(),
			name: r.text(),
			visible: r.flag(),
			opacity: r.byte(),
			blend: r.byte(),
			origin: [r.z(), r.z()],
			size: [r.u(2), r.u(1)],
			default: [...r.take(4)],
			filters: [],
			tiles: [],
		};
		assert.equal(r.u(1), i, 'one Invert on top root only');
		let field = null;
		if (i === 1) {
			assert.equal(r.id(), uuid('14'.repeat(16)), 'fixed Invert UUID');
			assert.equal(r.flag(), true, 'enabled Invert');
			assert.equal(r.u(0), 0);
			assert.equal(r.u(0), 0);
			field = ownMask(r);
		}
		assert.equal(r.flag(), false, 'no own mask on either root');
		assert.equal(r.byte(), 0, 'no raster block');
		assert.deepEqual(
			layer,
			expectedLayers[i],
			'preserve complete ordered raw layer',
		);
		layers.push({
			...layer,
			filters:
				i === 1
					? [
							{
								id: uuid('14'.repeat(16)),
								enabled: true,
								profile: 'invert-rgb-v1',
								parameters: '',
								mask: field,
							},
						]
					: [],
			mask: null,
		});
	}
	assert.equal(r.pos, r.bytes.length, 'exact body consumption');
	const mask = layers[1].filters[0].mask;
	const rgba = Buffer.from(
		[0, 1].flatMap((x) =>
			mask?.enabled && mask.coverage[x] === 0 ? unfiltered : original,
		),
	);
	const rawIdentity = {
		document,
		canvas: [2, 1],
		title: null,
		sourceMetadata: null,
		layers: layers.map(({ mask: _mask, ...layer }) => ({
			...layer,
			filters: layer.filters.map(
				({ mask: _filterMask, ...filter }) => filter,
			),
		})),
	};
	return {
		state,
		document,
		layers,
		mask,
		rgba,
		fullRGBA: Buffer.from(rgba),
		rawIdentity,
	};
}
/** Fixed PCND8 source derived from canonical A by adding top Invert14 only. */
export function fixture() {
	assert.equal(fs.statSync(source).size, 195);
	const bytes = fs.readFileSync(source);
	assert.equal(sha(bytes), SOURCE_SHA);
	const parsed = assertSaved(bytes, { mask: 'none' });
	assert.equal(parsed.state, uuid('11'.repeat(16)));
	return { bytes, sha256: SOURCE_SHA, canvas: [2, 1], ...parsed };
}
/** Only top filter mask changes; complete raw layers and Invert14 stay fixed. */
export function assertSaved(bytes, expected) {
	assert(expected && Object.getPrototypeOf(expected) === Object.prototype);
	assert.deepEqual(Object.keys(expected), ['mask']);
	assert(['none', 'right', 'disabled'].includes(expected.mask));
	const parsed = inspect(bytes);
	if (expected.mask === 'none') assert.equal(parsed.mask, null);
	else {
		assert(parsed.mask);
		assert.equal(parsed.mask.enabled, expected.mask === 'right');
		assert.deepEqual(parsed.mask.coverage, [0, 255]);
	}
	return parsed;
}
