// Independently authored PNG and container checks; no PaintCraft codecs.
import assert from 'node:assert/strict';
import { deflateSync } from 'node:zlib';
import { pngSamples } from './metadata-export-oracles.mjs';

function chunk(tag, payload) {
	const name = Buffer.from(tag);
	const body = Buffer.concat([name, payload]);
	let crc = 0xffffffff;
	for (const value of body) {
		crc ^= value;
		for (let bit = 0; bit < 8; bit++)
			crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
	}
	const size = Buffer.alloc(4);
	size.writeUInt32BE(payload.length);
	const sum = Buffer.alloc(4);
	sum.writeUInt32BE((crc ^ 0xffffffff) >>> 0);
	return Buffer.concat([size, body, sum]);
}
export function sourcePng(profile) {
	const width = 32;
	const height = 32;
	const rgba = Buffer.alloc(width * height * 4);
	const scanlines = Buffer.alloc((width * 3 + 1) * height);
	for (let y = 0; y < height; y++) {
		for (let x = 0; x < width; x++) {
			const rgb = [(x * 7 + y * 3) & 255, (y * 11) & 255, (x ^ y) * 7];
			rgba.set([...rgb, 255], (y * width + x) * 4);
			scanlines.set(rgb, y * (width * 3 + 1) + 1 + x * 3);
		}
	}
	const header = Buffer.alloc(13);
	header.writeUInt32BE(width);
	header.writeUInt32BE(height, 4);
	header[8] = 8;
	header[9] = 2;
	const resolution = Buffer.alloc(9);
	resolution.writeUInt32BE(3780);
	resolution.writeUInt32BE(3779, 4);
	resolution[8] = 1;
	const bytes = Buffer.concat([
		Buffer.from('89504e470d0a1a0a', 'hex'),
		chunk('IHDR', header),
		chunk(
			'iCCP',
			Buffer.concat([Buffer.from('original\0\0'), deflateSync(profile)]),
		),
		chunk('pHYs', resolution),
		chunk('IDAT', deflateSync(scanlines)),
		chunk('IEND', Buffer.alloc(0)),
	]);
	return { bytes, rgba, width, height };
}
function metadataAbsent(png) {
	let offset = 8;
	const tags = [];
	while (offset < png.length) {
		assert.ok(offset + 12 <= png.length);
		const size = png.readUInt32BE(offset);
		assert.ok(offset + 12 + size <= png.length);
		tags.push(png.subarray(offset + 4, offset + 8).toString());
		offset += size + 12;
	}
	assert.equal(offset, png.length);
	assert.equal(tags.at(-1), 'IEND');
	assert.ok(!tags.includes('iCCP') && !tags.includes('pHYs'));
	return tags;
}
export function iconSamples(bytes, kind, expected) {
	assert.equal(bytes.readUInt16LE(0), 0);
	assert.equal(bytes.readUInt16LE(2), kind === 'ico' ? 1 : 2);
	const count = bytes.readUInt16LE(4);
	assert.equal(count, 4);
	const entries = [];
	let offset = 6 + count * 16;
	for (let index = 0; index < count; index++) {
		const entry = 6 + index * 16;
		const width = bytes[entry] || 256;
		const height = bytes[entry + 1] || 256;
		assert.equal(width, [16, 32, 48, 256][index]);
		assert.equal(height, width);
		if (kind === 'cur') {
			assert.equal(bytes.readUInt16LE(entry + 4), 0);
			assert.equal(bytes.readUInt16LE(entry + 6), 0);
		}
		const length = bytes.readUInt32LE(entry + 8);
		assert.equal(bytes.readUInt32LE(entry + 12), offset);
		const png = bytes.subarray(offset, offset + length);
		assert.equal(png.length, length);
		assert.deepEqual(
			png.subarray(0, 8),
			Buffer.from('89504e470d0a1a0a', 'hex'),
		);
		const tags = metadataAbsent(png);
		const exact = width === 32 ? pngSamples(png, 32, 32, expected) : null;
		entries.push({ width, height, tags, exact });
		offset += length;
	}
	assert.equal(offset, bytes.length);
	return entries;
}
export function icnsSamples(bytes, expected) {
	assert.equal(bytes.subarray(0, 4).toString(), 'icns');
	assert.equal(bytes.readUInt32BE(4), bytes.length);
	assert.equal(bytes.subarray(8, 12).toString(), 'TOC ');
	assert.equal(bytes.readUInt32BE(12), 16);
	assert.equal(bytes.subarray(16, 20).toString(), 'ic11');
	assert.equal(bytes.readUInt32BE(20), bytes.length - 24);
	assert.equal(bytes.subarray(24, 28).toString(), 'ic11');
	assert.equal(bytes.readUInt32BE(28), bytes.length - 24);
	return pngSamples(bytes.subarray(32), 32, 32, expected);
}
