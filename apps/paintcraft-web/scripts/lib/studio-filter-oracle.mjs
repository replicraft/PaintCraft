// Independent bounded tiny-PCND reader and rational point-filter oracle.
// Qualification fixtures only: flat, opaque, origin-zero layers up to256×256.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { zstdDecompressSync } from 'node:zlib';
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
	assert.ok([1, 2, 3].includes(version));
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
	const presence = version === 3 ? take(1)[0] : Number(version === 2);
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
		if (version === 3) {
			const n = integer();
			assert.ok(n <= 256);
			for (let j = 0; j < n; j++) {
				const f = { id: id(), enabled: take(1)[0], kind: take(1)[0] };
				assert.ok(f.enabled <= 1);
				assert.ok([1, 2].includes(f.kind));
				if (f.kind === 2) {
					f.brightness = signed();
					f.contrast = signed();
					assert.ok(
						Math.abs(f.brightness) <= 100 &&
							Math.abs(f.contrast) <= 99,
					);
				}
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
	return doc;
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
export function evaluated(doc) {
	assert.equal(doc.layers.length, 1);
	const layer = doc.layers[0];
	assert.deepEqual(layer.origin, [0, 0]);
	assert.equal(layer.visible, 1);
	assert.equal(layer.opacity, 255);
	assert.deepEqual([layer.width, layer.height], [doc.width, doc.height]);
	const pixels = Buffer.from(layer.rgba, 'hex');
	for (const filter of layer.filters) {
		if (!filter.enabled) continue;
		for (let p = 0; p < pixels.length; p += 4)
			for (let c = 0; c < 3; c++)
				pixels[p + c] =
					filter.kind === 1
						? 255 - pixels[p + c]
						: adjust(
								pixels[p + c],
								filter.brightness,
								filter.contrast,
							);
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

// Independent screenshot projection for the tiny white/black canvas marker.
export async function canvasMarker(bytes, colors = [255, 0]) {
	assert.ok(
		colors.length === 2 &&
			colors.every((v) => Number.isInteger(v) && v >= 0 && v <= 255) &&
			colors[0] !== colors[1],
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
			pixels[offset] === v &&
			pixels[offset + 1] === v &&
			pixels[offset + 2] === v
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
		'Find literal white/black source marker outside floating controls',
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
