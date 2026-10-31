// Independent source facts for bounded browser fixtures, never copy authority.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { appearance, sha } from './studio-filter-oracle.mjs';
export const uuidBytes = (id) => Buffer.from(id.replaceAll('-', ''), 'hex');
const be = (n, bytes = 8, signed = false) => {
	const b = Buffer.alloc(bytes);
	if (bytes === 4) b.writeUInt32BE(n);
	else if (signed) b.writeBigInt64BE(BigInt(n));
	else b.writeBigUInt64BE(BigInt(n));
	return b;
};
export function inventory(doc, stackSha) {
	assert.equal(doc.layers.length, 1);
	const l = doc.layers[0];
	return sha(
		Buffer.concat([
			Buffer.from('paintcraft.filter-bake-witness/v1\0'),
			uuidBytes(doc.document),
			uuidBytes(doc.state),
			be(doc.width, 4),
			be(doc.height, 4),
			be(1),
			uuidBytes(l.id),
			Buffer.from(stackSha, 'hex'),
			Buffer.from([l.visible, l.opacity]),
			be(l.origin[0], 8, true),
			be(l.origin[1], 8, true),
			be(l.width, 4),
			be(l.height, 4),
			Buffer.from(l.default),
		]),
	);
}
export function spatialPlan(source, doc, stackSha, residentBytes = 262144) {
	const masks = doc.layers.flatMap((l) =>
		l.filters.map((f) => f.mask).filter(Boolean),
	);
	const bands = masks.reduce((n, m) => n + m.bands.length, 0),
		runs = masks.reduce(
			(n, m) => n + m.bands.reduce((n, b) => n + b.runs.length, 0),
			0,
		);
	const metadataBytes = doc.attachment
		? Number(Buffer.from(doc.attachment.header, 'hex').readBigUInt64BE(112))
		: 0;
	assert.ok(metadataBytes + 192 < 1048576);
	return {
		owner: {
			session: '44444444-4444-4444-8444-444444444444',
			state: doc.state,
			epoch: '0',
			frontend: 3,
		},
		draft: '55555555-5555-4555-8555-555555555555',
		document: doc.document,
		sourceSha256: sha(source),
		size: [doc.width, doc.height],
		layers: doc.layers.length,
		residentBytes,
		metadataBytes,
		metadataCharge: doc.attachment ? 1048576 : 0,
		maskCount: masks.length,
		maskBands: bands,
		maskRuns: runs,
		maskDecodeBytes: bands ? 2097152 : 0,
		filterDigest: inventory(doc, stackSha),
		protocol: 3,
		spatialCount: doc.layers
			.flatMap((l) => l.filters)
			.filter((f) => f.kind === 7).length,
		workspaceBytes: 64 * 1024 * 1024,
		workLimit: 268435456,
	};
}
export function rendered(doc) {
	assert.equal(doc.layers.length, 1);
	const l = doc.layers[0],
		pixels = appearance(l),
		out = Buffer.alloc(doc.width * doc.height * 4);
	assert.equal(l.visible, 1);
	assert.equal(l.opacity, 255);
	for (let y = 0; y < doc.height; y++)
		for (let x = 0; x < doc.width; x++) {
			const sx = x - l.origin[0],
				sy = y - l.origin[1];
			if (sx < 0 || sy < 0 || sx >= l.width || sy >= l.height) continue;
			const pixel = pixels.subarray(
				(sy * l.width + sx) * 4,
				(sy * l.width + sx) * 4 + 4,
			);
			if (pixel[3]) out.set(pixel, (y * doc.width + x) * 4);
		}
	return [...out];
}
export function nativeSource(ids) {
	const u = (n) => {
		const out = [];
		do {
			const b = n & 127;
			n = Math.floor(n / 128);
			out.push(b | (n ? 128 : 0));
		} while (n);
		return Buffer.from(out);
	};
	const name = Buffer.from('native-step-source');
	const body = Buffer.concat([
		Buffer.from('PCND\x01\r\n\x1a'),
		uuidBytes(ids.document),
		uuidBytes(ids.state),
		u(256),
		u(256),
		Buffer.from([0, 1]),
		uuidBytes(ids.layer),
		u(name.length),
		name,
		Buffer.from([1, 255, 0, 0]),
		u(256),
		u(256),
		Buffer.from([20, 40, 60, 255, 0]),
	]);
	const source = Buffer.concat([
		body,
		createHash('sha256').update(body).digest(),
	]);
	const stack = sha(
		Buffer.concat([
			Buffer.from('paintcraft.filter-stack/v1\0'),
			Buffer.from([0, 0]),
		]),
	);
	return { source, stack };
}
