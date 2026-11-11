// Independent bounded tile codecs. LZ4 ancestry is the pinned UI oracle, not Rust.
import assert from 'node:assert/strict';
import * as zlib from 'node:zlib';
export const MAX_TILE = 262144;
export function lz4(input, length) {
	assert.ok(length > 0 && length <= 393216);
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
	assert.equal(p, input.length);
	assert.equal(q, length);
	return out;
}
export function runs(input, length) {
	assert.ok(
		input.length > 0 && input.length <= 393216 && input.length % 6 === 0,
	);
	const out = Buffer.alloc(length);
	let p = 0;
	for (let i = 0; i < input.length; i += 6) {
		const n = input.readUInt16LE(i) + 1;
		assert.ok(p + n * 4 <= length);
		for (let j = 0; j < n; j++) {
			input.copy(out, p, i + 2, i + 6);
			p += 4;
		}
	}
	assert.equal(p, length);
	return out;
}
// One ordinary dictionary-free frame, no skippable/concatenated/suffix frames.
// Window is capped independently of the native decoder's allocation policy.
export function zstdFrame(input, length) {
	assert.ok(input.length >= 6 && input.length <= MAX_TILE);
	let p = 0;
	const take = (n) => {
		assert.ok(p + n <= input.length);
		const b = input.subarray(p, p + n);
		p += n;
		return b;
	};
	assert.equal(take(4).toString('hex'), '28b52ffd');
	const f = take(1)[0],
		single = Boolean(f & 32),
		checksum = Boolean(f & 4),
		flag = f >>> 6;
	assert.equal(f & 24, 0, 'reserved/unused frame bits');
	assert.equal(f & 3, 0, 'dictionary frame unsupported');
	let window;
	if (!single) {
		const d = take(1)[0],
			base = 2 ** (10 + (d >>> 3));
		window = base + (base / 8) * (d & 7);
		assert.ok(window <= 8 * 1024 * 1024, 'bounded zstd window');
	}
	const sizeBytes = flag === 0 ? (single ? 1 : 0) : [0, 2, 4, 8][flag];
	if (sizeBytes) {
		const b = take(sizeBytes);
		let n = 0n;
		for (let i = 0; i < b.length; i++) n |= BigInt(b[i]) << BigInt(8 * i);
		if (sizeBytes === 2) n += 256n;
		assert.equal(n, BigInt(length), 'zstd admitted content extent');
		if (single) window = Number(n);
	}
	assert.ok(window > 0 && window <= 8 * 1024 * 1024);
	for (;;) {
		const b = take(3),
			h = b[0] | (b[1] << 8) | (b[2] << 16);
		const last = h & 1,
			kind = (h >>> 1) & 3,
			size = h >>> 3;
		assert.ok(
			kind !== 3 && size <= 131072 && size <= window,
			'zstd block bound',
		);
		take(kind === 1 ? 1 : size);
		if (last) break;
	}
	if (checksum) take(4);
	assert.equal(p, input.length, 'exact single frame consumption');
}
export function block(kind, input, length) {
	assert.ok(
		Number.isSafeInteger(length) &&
			length > 0 &&
			length <= MAX_TILE &&
			length % 4 === 0,
	);
	assert.ok(input.length > 0 && input.length <= MAX_TILE);
	if (kind === 1) {
		assert.equal(input.length, length);
		return Buffer.from(input);
	}
	if (kind === 2) return runs(input, length);
	if (kind === 3) {
		assert.equal(
			typeof zlib.zstdDecompressSync,
			'function',
			'installed Node zstd capability required; no fallback',
		);
		zstdFrame(input, length);
		const out = zlib.zstdDecompressSync(input, { maxOutputLength: length });
		assert.equal(out.length, length);
		return out;
	}
	if (kind === 4) return lz4(input, length);
	assert.equal(kind, 5, 'unsupported tile codec');
	assert.ok(input.length >= 5);
	const n = input.readUInt32LE(0);
	assert.ok(n > 0 && n % 6 === 0 && n <= (length / 4) * 6);
	return runs(lz4(input.subarray(4), n), length);
}
