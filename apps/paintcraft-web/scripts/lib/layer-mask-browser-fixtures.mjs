// Finite own-mask browser oracle, not a general PCND reader or renderer.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';

const source = new URL(
	'../fixtures/studio-layer-mask-normal.pcnd',
	import.meta.url,
);
const SOURCE_SHA =
	'eff49defd1fd2ffcaf623a26a07239a951eb4cad46d662f90b05d965614d490a';
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
const uuid = (hex) =>
	hex.replace(/^(.{8})(.{4})(.{4})(.{4})(.{12})$/, '$1-$2-$3-$4-$5');
const document = uuid('a1'.repeat(16)),
	layerId = uuid('a3'.repeat(16));
const selection = Object.freeze([0, 255, 255, 0, 0, 0, 0, 0]);
const inverse = Object.freeze([255, 0, 0, 255, 255, 255, 255, 255]);
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

/** Inspect only this fixed one-layer/default-only PCND1 or masked PCND7 scene. */
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
	assert([1, 7].includes(version), 'finite fixture versions only');
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
			assert.equal(
				r.flag(),
				false,
				'no filter mask in this finite scene',
			);
			filters.push({
				id,
				enabled: true,
				profile: 'invert-rgb-v1',
				parameters: '',
			});
		}
	}
	const own = version === 7 ? mask(r) : null;
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
	const color = filters.length ? [191, 127, 63] : [64, 128, 192];
	const rgba = Buffer.from(
		Array.from({ length: 8 }, (_, i) => {
			const alpha = own?.enabled ? own.coverage[i] : 255;
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

/** Fresh owned Normal default-only input; mutation cannot damage later calls. */
export function fixture() {
	assert.equal(
		fs.statSync(source).size,
		116,
		'fixed source length before read',
	);
	const bytes = fs.readFileSync(source);
	assert.equal(sha(bytes), SOURCE_SHA);
	const parsed = inspectSaved(bytes);
	assert.equal(parsed.state, uuid('a2'.repeat(16)));
	assert.equal(parsed.mask, null);
	return {
		bytes,
		sha256: SOURCE_SHA,
		canvas: [4, 2],
		rgba: Buffer.from(parsed.rgba),
	};
}

/** Verify one literal own-mask state and optional Invert Apply; return its evidence. */
export function assertSaved(bytes, expected) {
	assert(expected && Object.getPrototypeOf(expected) === Object.prototype);
	assert.deepEqual(Object.keys(expected).sort(), ['invert', 'mask']);
	assert.equal(typeof expected.invert, 'boolean');
	assert(
		['none', 'selection', 'inverse', 'disabled'].includes(expected.mask),
	);
	assert(
		expected.mask !== 'none' || !expected.invert,
		'unmasked filter version is outside finite scope',
	);
	const parsed = inspectSaved(bytes);
	assert.equal(parsed.filters.length, Number(expected.invert));
	if (expected.mask === 'none') assert.equal(parsed.mask, null);
	else {
		assert(parsed.mask);
		assert.equal(parsed.mask.enabled, expected.mask !== 'disabled');
		assert.deepEqual(
			parsed.mask.coverage,
			expected.mask === 'selection' ? selection : inverse,
		);
	}
	return parsed;
}
