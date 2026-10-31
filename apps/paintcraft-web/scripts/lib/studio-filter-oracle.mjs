// Independent bounded tiny-PCND1..5 reader and point/spatial fixture oracle.
// Tiny flat qualification sources only; never a general interchange decoder.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { zstdDecompressSync } from 'node:zlib';
import { boxPlane, spatialMix } from './studio-box-oracle.mjs';
export const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
function lz4(input, length) {
	assert.ok(length <= 393216);
	const out = Buffer.alloc(length);
	let p = 0,
		q = 0;
	const extent = (initial) => {
		let n = initial;
		if (initial === 15) {
			let byte;
			do {
				assert.ok(p < input.length);
				byte = input[p++];
				n += byte;
			} while (byte === 255);
		}
		return n;
	};
	while (p < input.length) {
		const token = input[p++],
			literal = extent(token >>> 4);
		assert.ok(p + literal <= input.length && q + literal <= length);
		input.copy(out, q, p, p + literal);
		p += literal;
		q += literal;
		if (p === input.length) break;
		assert.ok(p + 2 <= input.length);
		const offset = input.readUInt16LE(p);
		p += 2;
		assert.ok(offset > 0 && offset <= q);
		const count = extent(token & 15) + 4;
		assert.ok(q + count <= length);
		for (let i = 0; i < count; i++) {
			out[q] = out[q - offset];
			q++;
		}
	}
	assert.equal(q, length);
	return out;
}
function runs(input, length) {
	assert.equal(input.length % 6, 0);
	const out = Buffer.alloc(length);
	let p = 0;
	for (let i = 0; i < input.length; i += 6) {
		const n = input.readUInt16LE(i) + 1;
		assert.ok(p + n * 4 <= length);
		for (let k = 0; k < n; k++) {
			input.copy(out, p, i + 2, i + 6);
			p += 4;
		}
	}
	assert.equal(p, length);
	return out;
}
function block(kind, input, length) {
	if (kind === 1) {
		assert.equal(input.length, length);
		return input;
	}
	if (kind === 2) return runs(input, length);
	if (kind === 3) {
		const out = zstdDecompressSync(input, { maxOutputLength: length });
		assert.equal(out.length, length);
		return out;
	}
	if (kind === 4) return lz4(input, length);
	assert.equal(kind, 5);
	const count = input.readUInt32LE(0);
	assert.ok(count > 0 && count <= (length / 4) * 6 && count % 6 === 0);
	return runs(lz4(input.subarray(4), count), length);
}
export function project(bytes) {
	assert.ok(bytes.length >= 43 && bytes.length <= 16 * 1024 * 1024);
	const version = bytes[4];
	assert.ok([1, 2, 3, 4, 5].includes(version));
	assert.equal(bytes.subarray(0, 4).toString(), 'PCND');
	assert.equal(bytes.subarray(5, 8).toString('hex'), '0d0a1a');
	assert.equal(
		sha(bytes.subarray(0, -32)),
		bytes.subarray(-32).toString('hex'),
	);
	let p = 8;
	const end = bytes.length - 32;
	const take = (n) => {
		assert.ok(Number.isSafeInteger(n) && n >= 0 && p + n <= end);
		const v = bytes.subarray(p, p + n);
		p += n;
		return v;
	};
	const integer = () => {
		let n = 0n;
		for (let i = 0; i < 10; i++) {
			const b = take(1)[0];
			n |= BigInt(b & 127) << BigInt(i * 7);
			if (b < 128) {
				assert.ok(i === 0 || b !== 0);
				assert.ok(n <= BigInt(Number.MAX_SAFE_INTEGER));
				return Number(n);
			}
		}
		throw Error('Varint bound');
	};
	const signed = () => {
		const v = integer();
		return v % 2 === 0 ? v / 2 : -(v + 1) / 2;
	};
	const string = () => {
		const n = integer();
		assert.ok(n <= 16384);
		return take(n).toString('utf8');
	};
	const id = () => {
		const h = take(16).toString('hex');
		return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
	};
	const doc = {
		version,
		document: id(),
		state: id(),
		width: integer(),
		height: integer(),
		title: null,
		layers: [],
	};
	const title = take(1)[0];
	assert.ok(title <= 1);
	if (title) doc.title = string();
	const presence = version >= 3 ? take(1)[0] : Number(version === 2);
	assert.ok(presence <= 1);
	const metadata = presence === 1;
	if (metadata) {
		const header = take(192);
		assert.equal(header.subarray(0, 8).toString(), 'PCSMETA1');
		const count = Number(header.readBigUInt64BE(112));
		assert.ok(count <= 8 * 1024 * 1024);
		const profile = take(count);
		doc.attachment = {
			header: header.toString('hex'),
			profileSha: sha(profile),
		};
	}
	let spatialCount = 0;
	let maskCount = 0,
		bandCount = 0,
		runCount = 0;
	const mask = () => {
		const present = take(1)[0];
		assert.ok(present <= 1);
		if (!present) return null;
		maskCount++;
		const enabled = take(1)[0];
		assert.ok(enabled <= 1);
		assert.equal(take(1)[0], 1);
		const domain = [signed(), signed(), signed(), signed()];
		const [left, top, right, bottom] = domain;
		assert.ok(left < right && top < bottom);
		assert.ok(domain.every((v) => Math.abs(v) <= 140_737_488_355_327));
		const n = integer(),
			bands = [];
		bandCount += n;
		assert.ok(bandCount <= 262144);
		let lastY = top;
		for (let i = 0; i < n; i++) {
			const y0 = top + integer(),
				y1 = y0 + integer(),
				count = integer(),
				runs = [];
			assert.ok(y0 >= lastY && y0 < y1 && y1 <= bottom);
			assert.ok(count > 0);
			runCount += count;
			assert.ok(runCount <= 262144);
			let lastX = left,
				lastCoverage = 0;
			for (let j = 0; j < count; j++) {
				const x0 = left + integer(),
					x1 = x0 + integer(),
					coverage = take(1)[0];
				assert.ok(x0 >= lastX && x0 < x1 && x1 <= right);
				assert.ok(coverage > 0);
				assert.ok(x0 !== lastX || coverage !== lastCoverage);
				runs.push({ x0, x1, coverage });
				lastX = x1;
				lastCoverage = coverage;
			}
			const previous = bands.at(-1);
			assert.ok(
				!previous ||
					previous.y1 !== y0 ||
					JSON.stringify(previous.runs) !== JSON.stringify(runs),
			);
			bands.push({ y0, y1, runs });
			lastY = y1;
		}
		return { enabled, domain, bands };
	};
	const count = integer();
	assert.ok(count <= 8);
	for (let i = 0; i < count; i++) {
		const layer = {
			id: id(),
			name: string(),
			visible: take(1)[0],
			opacity: take(1)[0],
			origin: [signed(), signed()],
			width: integer(),
			height: integer(),
			default: [...take(4)],
			filters: [],
		};
		assert.ok(
			layer.width > 0 &&
				layer.height > 0 &&
				layer.width <= 256 &&
				layer.height <= 256,
		);
		const pixels = Buffer.alloc(layer.width * layer.height * 4);
		for (let q = 0; q < pixels.length; q += 4) pixels.set(layer.default, q);
		if (version >= 3) {
			const n = integer();
			assert.ok(n <= 256);
			for (let j = 0; j < n; j++) {
				const f = { id: id(), enabled: take(1)[0], kind: take(1)[0] };
				assert.ok(f.enabled <= 1);
				assert.ok(
					[1, 2, 3, 4, 5, 6].includes(f.kind) ||
						(version === 5 && f.kind === 7),
				);
				if (f.kind === 7) {
					f.radius = take(1)[0];
					assert.ok(f.radius <= 32);
					spatialCount++;
				}
				if (f.kind === 2) {
					f.brightness = signed();
					f.contrast = signed();
					assert.ok(
						Math.abs(f.brightness) <= 100 &&
							Math.abs(f.contrast) <= 99,
					);
				}
				if (f.kind === 3) {
					f.strength = take(1)[0];
					assert.ok(f.strength <= 100);
				}
				if (f.kind === 4) f.level = take(1)[0];
				if (f.kind === 5) {
					f.levels = take(1)[0];
					assert.ok(f.levels >= 2 && f.levels <= 32);
				}
				if (version >= 4) f.mask = mask();
				layer.filters.push(f);
			}
		}
		for (;;) {
			const kind = take(1)[0];
			if (kind === 0) break;
			assert.ok(kind <= 5);
			const x = integer() * 256,
				y = integer() * 256,
				n = integer();
			assert.ok(x < layer.width && y < layer.height && n <= 262144);
			const w = Math.min(256, layer.width - x),
				h = Math.min(256, layer.height - y);
			const raw = block(kind, take(n), w * h * 4);
			for (let row = 0; row < h; row++)
				raw.copy(
					pixels,
					((y + row) * layer.width + x) * 4,
					row * w * 4,
					(row + 1) * w * 4,
				);
		}
		layer.rgba = pixels.toString('hex');
		doc.layers.push(layer);
	}
	assert.equal(p, end);
	assert.ok(version !== 4 || maskCount > 0);
	assert.ok(version !== 5 || spatialCount > 0);
	return doc;
}
export function maskCoverage(mask, x, y) {
	if (!mask?.enabled) return 255;
	const [left, top, right, bottom] = mask.domain;
	if (x < left || y < top || x >= right || y >= bottom) return 0;
	const band = mask.bands.find((b) => y >= b.y0 && y < b.y1);
	return band?.runs.find((r) => x >= r.x0 && x < r.x1)?.coverage ?? 0;
}
function add([an, ad], [bn, bd]) {
	return [an * bd + bn * ad, ad * bd];
}
function multiply([an, ad], [bn, bd]) {
	return [an * bn, ad * bd];
}
function adjust(value, b, c) {
	const centered = [BigInt(value) * 2n - 255n, 2n];
	const slope = [100n + BigInt(c), 100n - BigInt(c)];
	const fraction = add(add(multiply(centered, slope), [255n, 2n]), [
		255n * BigInt(b),
		100n,
	]);
	const [n, d] = fraction;
	if (n <= 0n) return 0;
	if (n >= 255n * d) return 255;
	return Number((n * 2n + d) / (d * 2n));
}
// Exact nonnegative rational rounding, with half ties upward.
function rounded(n, d) {
	assert.ok(n >= 0n && d > 0n);
	return Number((2n * n + d) / (2n * d));
}
function parameter(value, low, high) {
	assert.ok(Number.isInteger(value) && value >= low && value <= high);
	return BigInt(value);
}
function candidate(filter, rgb) {
	const luma = () =>
		2126n * BigInt(rgb[0]) + 7152n * BigInt(rgb[1]) + 722n * BigInt(rgb[2]);
	switch (filter.kind) {
		case 1:
			return rgb.map((v) => 255 - v);
		case 2:
			parameter(filter.brightness, -100, 100);
			parameter(filter.contrast, -99, 99);
			return rgb.map((v) =>
				adjust(v, filter.brightness, filter.contrast),
			);
		case 3: {
			const strength = parameter(filter.strength, 0, 100),
				y = luma();
			return rgb.map((v) =>
				rounded(
					(100n - strength) * 10000n * BigInt(v) + strength * y,
					1000000n,
				),
			);
		}
		case 4:
			return Array(3).fill(
				luma() >= 10000n * parameter(filter.level, 0, 255) ? 255 : 0,
			);
		case 5: {
			const k = parameter(filter.levels, 2, 32) - 1n;
			return rgb.map((v) =>
				rounded(255n * BigInt(rounded(BigInt(v) * k, 255n)), k),
			);
		}
		case 6:
			return rgb.map((v) => (v >= 128 ? 255 - v : v));
		default:
			throw Error('Unknown point-filter algorithm/version');
	}
}
export function evaluated(doc) {
	assert.equal(doc.layers.length, 1);
	const layer = doc.layers[0];
	if (layer.filters.some((f) => f.kind === 7))
		assert.equal(
			doc.version,
			5,
			'Unknown point-filter algorithm/version outside PCND5',
		);
	assert.deepEqual(layer.origin, [0, 0]);
	assert.equal(layer.visible, 1);
	assert.equal(layer.opacity, 255);
	assert.deepEqual([layer.width, layer.height], [doc.width, doc.height]);
	return appearance(layer);
}
export function appearance(layer) {
	let pixels = Buffer.from(layer.rgba, 'hex');
	for (const filter of layer.filters) {
		if (!filter.enabled) continue;
		if (filter.kind === 7) {
			const candidate = boxPlane(
				pixels,
				layer.width,
				layer.height,
				filter.radius,
			);
			const output = Buffer.alloc(pixels.length);
			for (let p = 0; p < pixels.length; p += 4) {
				const m = maskCoverage(
					filter.mask,
					(p / 4) % layer.width,
					Math.floor(p / 4 / layer.width),
				);
				output.set(
					spatialMix(
						pixels.subarray(p, p + 4),
						candidate.subarray(p, p + 4),
						m,
					),
					p,
				);
			}
			pixels = output;
			continue;
		}
		for (let p = 0; p < pixels.length; p += 4) {
			const coverage = maskCoverage(
				filter.mask,
				(p / 4) % layer.width,
				Math.floor(p / 4 / layer.width),
			);
			const incomingRgb = [...pixels.subarray(p, p + 3)],
				candidateRgb = candidate(filter, incomingRgb);
			for (let c = 0; c < 3; c++) {
				const incoming = incomingRgb[c],
					candidate = candidateRgb[c];
				pixels[p + c] = Math.floor(
					((255 - coverage) * incoming + coverage * candidate + 127) /
						255,
				);
			}
		}
	}
	return pixels;
}
export function rawSemantics(doc) {
	return {
		document: doc.document,
		width: doc.width,
		height: doc.height,
		title: doc.title,
		attachment: doc.attachment,
		layers: doc.layers.map(({ filters, ...layer }) => layer),
	};
}

// Independent screenshot projection for the tiny two-color canvas marker.
export async function canvasMarker(bytes, colors = [255, 0]) {
	colors = colors.map((v) => (typeof v === 'number' ? [v, v, v] : v));
	assert.ok(
		colors.length === 2 &&
			colors.every(
				(v) =>
					Array.isArray(v) &&
					v.length === 3 &&
					v.every((c) => Number.isInteger(c) && c >= 0 && c <= 255),
			) &&
			colors[0].join(',') !== colors[1].join(','),
	);
	const [firstColor, secondColor] = colors;
	const { inflateSync } = await import('node:zlib');
	assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
	let p = 8,
		width,
		height,
		channels;
	const parts = [];
	while (p < bytes.length) {
		const n = bytes.readUInt32BE(p),
			tag = bytes.subarray(p + 4, p + 8).toString(),
			data = bytes.subarray(p + 8, p + 8 + n);
		assert.ok(p + 12 + n <= bytes.length);
		if (tag === 'IHDR') {
			width = data.readUInt32BE(0);
			height = data.readUInt32BE(4);
			assert.equal(data[8], 8);
			assert.ok([2, 6].includes(data[9]));
			assert.equal(data[12], 0);
			channels = data[9] === 2 ? 3 : 4;
		}
		if (tag === 'IDAT') parts.push(data);
		p += n + 12;
	}
	assert.ok(width * height <= 4_194_304);
	const stride = width * channels,
		raw = inflateSync(Buffer.concat(parts), {
			maxOutputLength: (stride + 1) * height,
		}),
		pixels = Buffer.alloc(stride * height);
	assert.equal(raw.length, (stride + 1) * height);
	for (let y = 0; y < height; y++) {
		const filter = raw[y * (stride + 1)];
		assert.ok(filter <= 4);
		for (let x = 0; x < stride; x++) {
			const a = x >= channels ? pixels[y * stride + x - channels] : 0,
				b = y ? pixels[(y - 1) * stride + x] : 0,
				c =
					y && x >= channels
						? pixels[(y - 1) * stride + x - channels]
						: 0;
			const prediction = a + b - c,
				distances = [
					Math.abs(prediction - a),
					Math.abs(prediction - b),
					Math.abs(prediction - c),
				];
			const paeth =
				distances[0] <= distances[1] && distances[0] <= distances[2]
					? a
					: distances[1] <= distances[2]
						? b
						: c;
			pixels[y * stride + x] =
				(raw[y * (stride + 1) + 1 + x] +
					[0, a, b, Math.floor((a + b) / 2), paeth][filter]) &
				255;
		}
	}
	const color = (x, y, v) => {
		if (x < 0 || y < 0 || x >= width || y >= height) return false;
		const offset = y * stride + x * channels;
		return (
			pixels[offset] === v[0] &&
			pixels[offset + 1] === v[1] &&
			pixels[offset + 2] === v[2]
		);
	};
	let best = null;
	for (let y = 150; y < Math.min(height, 470); y++)
		for (let x = 145; x < Math.min(width - 2, 1050); x++) {
			if (!color(x, y, firstColor) || color(x - 1, y, firstColor))
				continue;
			let white = 0,
				black = 0;
			while (white < 65 && color(x + white, y, firstColor)) white++;
			while (black < 65 && color(x + white + black, y, secondColor))
				black++;
			if (
				white < 3 ||
				black < 3 ||
				white > 64 ||
				black > 64 ||
				Math.abs(white - black) > 2
			)
				continue;
			let tall = 0;
			for (; tall < Math.max(white, black) + 3; tall++) {
				let okay = true;
				for (let dx = 0; dx < white + black; dx++)
					if (
						!color(
							x + dx,
							y + tall,
							dx < white ? firstColor : secondColor,
						)
					) {
						okay = false;
						break;
					}
				if (!okay) break;
			}
			if (tall < Math.min(white, black) - 1) continue;
			const area = (white + black) * tall;
			if (!best || area > best.area)
				best = { x, y, width: white + black, height: tall, area };
		}
	assert.ok(
		best,
		'Find literal two-color source marker outside floating controls',
	);
	return best;
}

export function assetInventory(base) {
	const assets = {};
	function visit(folder) {
		for (const row of fs.readdirSync(folder, { withFileTypes: true })) {
			const file = path.join(folder, row.name);
			if (row.isDirectory()) visit(file);
			else
				assets[path.relative(base, file)] = {
					bytes: fs.statSync(file).size,
					sha256: sha(fs.readFileSync(file)),
				};
		}
	}
	visit(base);
	return assets;
}
