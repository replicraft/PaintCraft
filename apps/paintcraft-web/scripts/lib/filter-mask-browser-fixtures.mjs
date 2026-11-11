// Finite Normal filter-mask oracle; existing own-mask helpers are unchanged.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';

const source = new URL(
	'../fixtures/studio-layer-mask-saved-vectors.json',
	import.meta.url,
);
const SOURCE_SHA =
	'd2a386ae158305dbcd1ee2917da8a2be3457a33fcc3452e7c9de7dc079a514d1';
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
const uuid = (hex) =>
	hex.replace(/^(.{8})(.{4})(.{4})(.{4})(.{12})$/, '$1-$2-$3-$4-$5');
const document = uuid('a1'.repeat(16)),
	layerId = uuid('a3'.repeat(16));
const selection = Object.freeze([0, 255, 255, 0, 0, 0, 0, 0]);
const filterCoverage = Object.freeze([0, 255, 0, 0, 0, 0, 0, 0]);
const utf8 = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

class Reader {
	constructor(bytes) {
		this.bytes = bytes;
		this.pos = 0;
	}
	take(length) {
		assert(
			Number.isSafeInteger(length) &&
				length >= 0 &&
				this.pos + length <= this.bytes.length,
			'finite field extent',
		);
		const bytes = this.bytes.subarray(this.pos, this.pos + length);
		this.pos += length;
		return bytes;
	}
	byte() {
		return this.take(1)[0];
	}
	u(maximum) {
		let value = 0;
		for (let i = 0; i < 2; i++) {
			const byte = this.byte();
			value += (byte & 127) * 2 ** (7 * i);
			if (byte < 128) {
				assert(
					(i === 0 || byte !== 0) && value <= maximum,
					'canonical bounded integer',
				);
				return value;
			}
		}
		throw Error('finite integer is too long');
	}
	z() {
		const value = this.u(16);
		return value % 2 ? -(value + 1) / 2 : value / 2;
	}
	flag() {
		const value = this.byte();
		assert(value <= 1);
		return Boolean(value);
	}
	text() {
		return utf8.decode(this.take(this.u(64)));
	}
	id() {
		return uuid(this.take(16).toString('hex'));
	}
	end() {
		assert.equal(
			this.pos,
			this.bytes.length,
			'exact finite body consumption',
		);
	}
}
function mask(r) {
	if (!r.flag()) return null;
	const enabled = r.flag();
	assert.equal(r.byte(), 1, 'only canonical A8 bands');
	const domain = [r.z(), r.z(), r.z(), r.z()];
	assert.deepEqual(domain, [0, 0, 4, 2], 'fixed layer domain');
	const bands = [],
		coverage = Array(8).fill(0),
		count = r.u(2);
	let lastY = 0;
	for (let i = 0; i < count; i++) {
		const y0 = r.u(1),
			y1 = y0 + r.u(2),
			n = r.u(2),
			runs = [];
		assert(lastY <= y0 && y0 < y1 && y1 <= 2 && n > 0);
		let lastX = 0,
			lastCoverage = 0;
		for (let j = 0; j < n; j++) {
			const x0 = r.u(3),
				x1 = x0 + r.u(4),
				value = r.byte();
			assert(lastX <= x0 && x0 < x1 && x1 <= 4 && value > 0);
			assert(
				x0 !== lastX || value !== lastCoverage,
				'adjacent equal runs must merge',
			);
			runs.push({ x0, x1, coverage: value });
			for (let y = y0; y < y1; y++)
				for (let x = x0; x < x1; x++) coverage[y * 4 + x] = value;
			lastX = x1;
			lastCoverage = value;
		}
		const previous = bands.at(-1);
		assert(
			!previous ||
				previous.y1 !== y0 ||
				JSON.stringify(previous.runs) !== JSON.stringify(runs),
			'adjacent equal bands must merge',
		);
		bands.push({ y0, y1, runs });
		lastY = y1;
	}
	return { enabled, domain, bands, coverage };
}

/** Inspect only this fixed one-layer Invert/own-mask-R PCND7 scene. */
export function inspectSaved(value) {
	assert(
		value instanceof Uint8Array &&
			value.byteLength >= 43 &&
			value.byteLength <= 4096,
		'bounded saved byte view before copy',
	);
	const bytes = Buffer.from(value),
		version = bytes[4];
	assert.equal(bytes.subarray(0, 4).toString('hex'), '50434e44');
	assert.equal(version, 7, 'only PCND7 filter+own-mask fixture');
	assert.equal(bytes.subarray(5, 8).toString('hex'), '0d0a1a');
	assert.equal(
		sha(bytes.subarray(0, -32)),
		bytes.subarray(-32).toString('hex'),
		'authenticated saved stream',
	);
	const r = new Reader(bytes.subarray(8, -32));
	let catalog = 0;
	if (version === 7) {
		const prelude = new Reader(r.take(r.u(64)));
		catalog = prelude.u(1);
		if (catalog) {
			assert.equal(prelude.text(), 'invert-rgb-v1');
			assert.equal(prelude.text(), 'params-v1');
		}
		prelude.end();
	}
	assert.equal(r.id(), document, 'fixed document identity');
	const state = r.id();
	assert.notEqual(
		state,
		uuid('00'.repeat(16)),
		'real nonzero state identity',
	);
	assert.deepEqual([r.u(4), r.u(2)], [4, 2]);
	assert.equal(r.flag(), false, 'no title in finite source');
	if (version === 7)
		assert.equal(r.flag(), false, 'no source attachment in finite source');
	assert.equal(r.u(1), 1, 'one flat root');
	assert.equal(r.id(), layerId);
	assert.equal(r.text(), 'Mask subject');
	assert.equal(r.flag(), true);
	assert.equal(r.byte(), 255);
	assert.deepEqual([r.z(), r.z()], [0, 0]);
	assert.deepEqual([r.u(4), r.u(2)], [4, 2]);
	assert.deepEqual([...r.take(4)], [64, 128, 192, 255]);
	const filters = [];
	if (version === 7) {
		const count = r.u(1);
		assert.equal(count, catalog, 'exact required catalog use');
		if (count) {
			const id = r.id();
			assert.notEqual(id, uuid('00'.repeat(16)));
			assert.equal(r.flag(), true);
			assert.equal(r.u(0), 0);
			assert.equal(r.u(0), 0);
			const filterMask = mask(r);
			filters.push({
				id,
				enabled: true,
				profile: 'invert-rgb-v1',
				parameters: '',
				mask: filterMask,
			});
		}
	}
	assert.equal(filters.length, 1, 'exactly one Invert filter');
	assert.equal(filters[0].id, uuid('a4'.repeat(16)), 'preserve filter UUID');
	const own = mask(r);
	assert(own?.enabled);
	assert.deepEqual(own.coverage, selection, 'unchanged own-mask R');
	assert(version !== 7 || own !== null, 'PCND7 requires a stored own mask');
	assert.equal(r.byte(), 0, 'default-only scene; raster blocks unsupported');
	r.end();
	const rawRGBA = Buffer.from(Array(8).fill([64, 128, 192, 255]).flat());
	const rawIdentity = {
		document,
		canvas: [4, 2],
		title: null,
		sourceMetadata: null,
		layer: {
			id: layerId,
			name: 'Mask subject',
			visible: true,
			opacity: 255,
			origin: [0, 0],
			size: [4, 2],
			default: [64, 128, 192, 255],
		},
		rawSha256: sha(rawRGBA),
	};
	// Finite opaque/default source; this is not a general compositor.
	const fm = filters[0].mask;
	const rgba = Buffer.from(
		Array.from({ length: 8 }, (_, i) => {
			const alpha = own?.enabled ? own.coverage[i] : 255;
			const inverted = !fm?.enabled || fm.coverage[i] === 255;
			const color = inverted ? [191, 127, 63] : [64, 128, 192];
			return alpha ? [...color, alpha] : [0, 0, 0, 0];
		}).flat(),
	);
	return {
		version,
		document,
		state,
		layerId,
		mask: own,
		filters,
		rawIdentity,
		rawRGBA,
		rgba,
	};
}

/** Fresh owned tracked Invert+own-mask-R source; fixed document/state/filter UUIDs. */
export function fixture() {
	assert(
		fs.statSync(source).size <= 16384,
		'bounded tracked literal catalog',
	);
	const catalog = JSON.parse(fs.readFileSync(source));
	assert(Array.isArray(catalog.cases) && catalog.cases.length <= 16);
	const matches = catalog.cases.filter(
		(r) => r.mask === 'selection' && r.invert === true,
	);
	assert.equal(matches.length, 1);
	const row = matches[0];
	assert(
		typeof row.hex === 'string' &&
			row.hex.length <= 2048 &&
			/^[0-9a-f]+$/.test(row.hex),
	);
	const bytes = Buffer.from(row.hex, 'hex');
	assert.equal(sha(bytes), SOURCE_SHA);
	const parsed = assertSaved(bytes, { mask: 'none' });
	assert.equal(parsed.state, uuid('a2'.repeat(16)));
	return { bytes, sha256: SOURCE_SHA, canvas: [4, 2], ...parsed };
}

/** Only the filter mask may vary; raw source, own-mask R and filter UUID stay fixed. */
export function assertSaved(bytes, expected) {
	assert(expected && Object.getPrototypeOf(expected) === Object.prototype);
	assert.deepEqual(Object.keys(expected), ['mask']);
	assert(['none', 'selection', 'disabled'].includes(expected.mask));
	const parsed = inspectSaved(bytes);
	const field = parsed.filters[0].mask;
	if (expected.mask === 'none') assert.equal(field, null);
	else {
		assert(field);
		assert.equal(field.enabled, expected.mask === 'selection');
		assert.deepEqual(field.coverage, filterCoverage);
	}
	return {
		...parsed,
		filterMask: field,
		ownMask: parsed.mask,
		fullRGBA: Buffer.from(parsed.rgba),
	};
}
