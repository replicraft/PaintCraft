// Independent small-fixture projections, no PaintCraft codec or runtime bridge.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { inflateSync } from 'node:zlib';

const digest = (bytes) => createHash('sha256').update(bytes).digest();
export function projectAttachment(bytes, source, profile, width, height) {
	assert.deepEqual(
		bytes.subarray(0, 8),
		Buffer.from('50434e44020d0a1a', 'hex'),
	);
	assert.deepEqual(digest(bytes.subarray(0, -32)), bytes.subarray(-32));
	let offset = 40;
	const integer = () => {
		let n = 0;
		let shift = 0;
		for (let i = 0; i < 8; i++) {
			const b = bytes[offset++];
			assert.notEqual(b, undefined);
			n += (b & 127) * 2 ** shift;
			if (b < 128) return n;
			shift += 7;
		}
		throw Error('Fixture varint extent');
	};
	assert.equal(integer(), width);
	assert.equal(integer(), height);
	const title = bytes[offset++];
	assert.ok(title === 0 || title === 1);
	if (title) {
		const length = integer();
		offset += length;
	}
	const header = bytes.subarray(offset, offset + 192);
	assert.equal(header.length, 192);
	assert.equal(header.subarray(0, 8).toString(), 'PCSMETA1');
	assert.equal(header.readUInt16BE(8), 1);
	assert.equal(header[10], 1, 'Original PNG format');
	assert.equal(header[11], 0, 'No container selection');
	assert.equal(header.readBigUInt64BE(12), BigInt(source.length));
	assert.deepEqual(header.subarray(20, 52), digest(source));
	assert.deepEqual(
		[56, 60, 64, 68].map((p) => header.readUInt32BE(p)),
		[width, height, width, height],
	);
	assert.equal(header[72], 0, 'PNG has no source orientation');
	assert.equal(header[73], 0, 'No orientation transform');
	assert.equal(header[74], 1);
	assert.equal(header[75], 3, 'Original pixels per metre, not active DPI');
	assert.equal(header.readBigInt64BE(76), 3780n);
	assert.equal(header.readBigUInt64BE(84), 1n);
	assert.equal(header.readBigInt64BE(92), 3779n);
	assert.equal(header.readBigUInt64BE(100), 1n);
	assert.equal(header[108], 1);
	assert.equal(header.readUInt32BE(184), 1, 'Original PNG pHYs unit');
	assert.equal(header[109], 1);
	assert.equal(header[110], 2, 'RGB source model');
	assert.equal(header.readBigUInt64BE(112), BigInt(profile.length));
	const retained = bytes.subarray(
		offset + 192,
		offset + 192 + profile.length,
	);
	assert.deepEqual(
		retained,
		profile,
		'Exact original ICC bytes, not a color transform',
	);
	assert.deepEqual(header.subarray(120, 152), digest(profile));
	const unhashed = Buffer.from(header);
	unhashed.fill(0, 152, 184);
	assert.deepEqual(
		header.subarray(152, 184),
		digest(Buffer.concat([unhashed, profile])),
	);
	return {
		sourceSha256: digest(source).toString('hex'),
		profileSha256: digest(profile).toString('hex'),
		attachmentDigest: header.subarray(152, 184).toString('hex'),
		width,
		height,
		resolution: [3780, 3779, 'metre'],
	};
}
function crc32(bytes) {
	let crc = 0xffffffff;
	for (const b of bytes) {
		crc ^= b;
		for (let i = 0; i < 8; i++)
			crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
	}
	return (crc ^ 0xffffffff) >>> 0;
}
export function pngSamples(bytes, width, height, expected) {
	assert.deepEqual(
		bytes.subarray(0, 8),
		Buffer.from('89504e470d0a1a0a', 'hex'),
	);
	const tags = [];
	const data = [];
	let offset = 8;
	while (offset < bytes.length) {
		assert.ok(offset + 12 <= bytes.length);
		const size = bytes.readUInt32BE(offset);
		assert.ok(offset + 12 + size <= bytes.length);
		const tag = bytes.subarray(offset + 4, offset + 8).toString();
		const payload = bytes.subarray(offset + 8, offset + 8 + size);
		assert.equal(
			crc32(bytes.subarray(offset + 4, offset + 8 + size)),
			bytes.readUInt32BE(offset + 8 + size),
		);
		tags.push(tag);
		if (tag === 'IHDR') {
			assert.equal(payload.readUInt32BE(0), width);
			assert.equal(payload.readUInt32BE(4), height);
			assert.deepEqual([...payload.subarray(8)], [8, 6, 0, 0, 0]);
		}
		if (tag === 'IDAT') data.push(payload);
		offset += 12 + size;
	}
	assert.equal(tags.at(-1), 'IEND');
	assert.ok(
		!tags.includes('iCCP') && !tags.includes('pHYs'),
		'Original ICC and resolution omitted from the copy',
	);
	const stride = width * 4;
	const raw = inflateSync(Buffer.concat(data), {
		maxOutputLength: (stride + 1) * height,
	});
	assert.equal(raw.length, (stride + 1) * height);
	const rgba = Buffer.alloc(stride * height);
	for (let y = 0; y < height; y++) {
		const filter = raw[y * (stride + 1)];
		assert.ok(filter <= 4);
		for (let x = 0; x < stride; x++) {
			const left = x >= 4 ? rgba[y * stride + x - 4] : 0;
			const up = y ? rgba[(y - 1) * stride + x] : 0;
			const corner = y && x >= 4 ? rgba[(y - 1) * stride + x - 4] : 0;
			const p = left + up - corner;
			const [a, b, c] = [
				Math.abs(p - left),
				Math.abs(p - up),
				Math.abs(p - corner),
			];
			const paeth = a <= b && a <= c ? left : b <= c ? up : corner;
			const predictor = [0, left, up, Math.floor((left + up) / 2), paeth][
				filter
			];
			rgba[y * stride + x] =
				(raw[y * (stride + 1) + 1 + x] + predictor) & 255;
		}
	}
	assert.deepEqual(
		rgba,
		expected,
		'All independently authored source samples unchanged',
	);
	return {
		rgbaSha256: digest(rgba).toString('hex'),
		width,
		height,
		chunks: tags,
	};
}
