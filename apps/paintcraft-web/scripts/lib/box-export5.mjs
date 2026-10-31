// Captured actual Main-Wasm request5: never manufacture same-ABI quote facts.
import assert from 'node:assert/strict';
import { inventory } from './spatial-worker-fixtures.mjs';
import { project, rawSemantics, sha } from './studio-filter-oracle.mjs';

const uuid = (b) => {
	const h = Buffer.from(b).toString('hex');
	return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
};
export function capturedExport5(bytes, kind, original, stackSha, settings) {
	const b = Buffer.from(bytes),
		v = new DataView(b.buffer, b.byteOffset, b.length);
	assert.ok(['dds', 'msp'].includes(kind));
	assert.equal(
		b.subarray(0, 8).toString(),
		kind === 'dds' ? 'PCDDSQ05' : 'PCMSPQ05',
	);
	assert.equal(v.getUint16(8), 5);
	assert.equal(b.length, 320 + v.getUint32(12));
	assert.deepEqual([v.getUint32(16), v.getUint32(20)], [2, 1]);
	const source = b.subarray(320),
		doc = project(source);
	assert.equal(doc.version, 5);
	assert.deepEqual(rawSemantics(doc), rawSemantics(original));
	assert.deepEqual(doc.layers[0].filters, original.layers[0].filters);
	assert.equal(doc.state, original.state);
	assert.equal(b.subarray(81, 113).toString('hex'), sha(source));
	assert.equal(uuid(b.subarray(40, 56)), doc.state);
	assert.equal(b[64], 3);
	assert.ok(v.getBigUint64(56) >= 0n);
	assert.match(uuid(b.subarray(24, 40)), /^[\da-f-]{36}$/);
	assert.notEqual(b.subarray(65, 81).toString('hex'), '00'.repeat(16));
	assert.equal(b[122], 1);
	assert.equal(
		b.subarray(128, 160).toString('hex'),
		inventory(doc, stackSha),
	);
	assert.equal(uuid(b.subarray(160, 176)), doc.document);
	assert.equal(b.subarray(176, 209).toString('hex'), '00'.repeat(33));
	assert.equal(b[209], kind === 'dds' ? 1 : 2);
	assert.equal(b[210], 1);
	assert.equal(b[211], 0);
	assert.equal(v.getUint32(212), 0);
	assert.equal(v.getBigUint64(216), 0n);
	assert.equal(b.subarray(224, 256).toString('hex'), '00'.repeat(32));
	if (kind === 'dds') {
		assert.deepEqual([...b.subarray(113, 121)], settings);
		assert.equal(b[121], 4);
	} else {
		assert.equal(b[113], 2);
		assert.equal(b[114], 4);
	}
	const context = new DataView(b.buffer, b.byteOffset + 256, 64);
	assert.equal(context.getUint16(0), 5);
	assert.equal(context.getUint16(2), 1);
	const stored = doc.layers
			.flatMap((l) => l.filters)
			.filter((f) => f.kind === 7),
		active = stored.filter((f) => f.enabled);
	assert.equal(context.getUint32(4), stored.length);
	assert.equal(context.getUint32(8), active.length);
	assert.equal(context.getUint32(12), 0);
	assert.equal(context.getBigUint64(16), 1n);
	assert.ok(
		context.getBigUint64(24) > 0n &&
			context.getBigUint64(24) <= 64n * 1024n * 1024n,
	);
	assert.ok(
		context.getBigUint64(32) > 0n && context.getBigUint64(32) <= 268435456n,
	);
	assert.ok(
		context.getBigUint64(40) >=
			BigInt(source.length) + context.getBigUint64(24),
	);
	assert.ok(
		context.getBigUint64(40) <=
			(kind === 'dds' ? 192n : 160n) * 1024n * 1024n,
	);
	assert.equal(b.subarray(304, 320).toString('hex'), '00'.repeat(16));
	return {
		sourceSha256: sha(source),
		requestSha256: sha(b),
		state: doc.state,
		owner: uuid(b.subarray(24, 40)),
		epoch: v.getBigUint64(56).toString(),
		nonce: uuid(b.subarray(65, 81)),
		settings: [...b.subarray(113, 121)],
		spatialContext: b.subarray(256, 320).toString('hex'),
		spatialUnits: context.getBigUint64(32).toString(),
	};
}
// page.evaluate function: exact echo/extent/hash/pixel checks, fresh real Worker.
export async function replayExport5({
	request,
	kind,
	runtime,
	expected,
	mutation = null,
}) {
	const equal = (a, b, label) => {
		if (JSON.stringify(a) !== JSON.stringify(b))
			throw Error(
				`${label}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`,
			);
	};
	const input = Uint8Array.from(request),
		dv = new DataView(input.buffer);
	if (mutation === 'bake') input[122] = 0;
	if (mutation === 'source') input[81] ^= 1;
	if (mutation === 'owner') input[40] ^= 1;
	if (mutation === 'stored') dv.setUint32(260, dv.getUint32(260) + 1);
	if (mutation === 'workspace')
		dv.setBigUint64(280, dv.getBigUint64(280) + 1n);
	if (mutation === 'reserved') input[268] = 1;
	if (mutation === 'old-version') dv.setUint16(8, 4);
	if (mutation === 'future-version') dv.setUint16(8, 6);
	const header = input.slice(0, 320),
		worker = new Worker(runtime, { type: 'module' });
	let timer;
	const digest = async (b) =>
		[...new Uint8Array(await crypto.subtle.digest('SHA-256', b))]
			.map((n) => n.toString(16).padStart(2, '0'))
			.join('');
	try {
		const response = await new Promise((resolve, reject) => {
			timer = setTimeout(() => reject(Error('Export5 deadline')), 30000);
			worker.onmessage = ({ data }) => resolve(data);
			worker.onerror = (e) => reject(Error(e.message));
			worker.postMessage(
				{ kind: `${kind}-spatial-export`, bytes: input.buffer },
				[input.buffer],
			);
		});
		if (response.kind === 'failure') {
			if (
				!['reserved', 'old-version', 'future-version'].includes(
					mutation,
				)
			)
				throw Error(JSON.stringify(response));
			equal(
				response.code,
				'CodecFailed',
				'isolated malformed-header failure',
			);
			equal(response.bytes, undefined, 'no malformed-header artifact');
			return {
				mutation,
				kind: 'failure',
				code: response.code,
				scope: 'framing refused; runtime intentionally exposes generic isolated failure',
			};
		}
		equal(response.kind, 'result', 'completed response');
		const bytes = new Uint8Array(response.bytes),
			v = new DataView(bytes.buffer, bytes.byteOffset, bytes.length),
			slice = (b, a, z) => [...b.subarray(a, z)];
		equal(
			new TextDecoder().decode(bytes.subarray(0, 8)),
			kind === 'dds' ? 'PCDDSR05' : 'PCMSPR05',
			'magic',
		);
		equal(v.getUint16(8), 5, 'version');
		for (const [a, z, start] of [
			[32, 73, 24],
			[73, 89, 65],
			[89, 121, 81],
			[288, 320, 128],
			[320, 384, 160],
			[384, 416, 224],
			[416, 480, 256],
		])
			equal(
				slice(bytes, a, z),
				slice(header, start, start + z - a),
				`bound fields${a}`,
			);
		equal(
			slice(bytes, 121, 129),
			kind === 'dds' ? slice(header, 113, 121) : [2, 0, 0, 0, 0, 0, 0, 0],
			'settings',
		);
		equal(
			[
				v.getUint32(12),
				v.getUint32(16),
				v.getUint32(20),
				bytes[265],
				bytes[266],
			],
			[2, 1, request.length - 320, 4, header[122]],
			'source/shape/flags/intent',
		);
		const encodedSize = v.getUint32(24),
			width = v.getUint16(28),
			height = v.getUint16(30),
			detail = v.getUint16(263),
			previewSize = width * height * 4;
		equal(
			bytes.length,
			480 + encodedSize + previewSize + detail,
			'exact extent',
		);
		const encoded = bytes.slice(480, 480 + encodedSize),
			preview = bytes.slice(
				480 + encodedSize,
				480 + encodedSize + previewSize,
			),
			hex = (b) =>
				[...b].map((n) => n.toString(16).padStart(2, '0')).join('');
		equal(
			await digest(encoded),
			hex(bytes.subarray(129, 161)),
			'encoded SHA',
		);
		equal(
			await digest(preview),
			hex(bytes.subarray(161, 193)),
			'preview SHA',
		);
		if (mutation) {
			if (bytes[10] === 0) throw Error(`Accepted forged ${mutation}`);
			equal(
				[encodedSize, previewSize, bytes[193]],
				[0, 0, 0],
				'refusal no artifact',
			);
			if (!detail) throw Error('Missing typed refusal detail');
			if (mutation === 'bake') equal(bytes[10], 1, 'BakeRefuse');
		} else {
			equal(bytes[10], 0, 'success');
			equal([width, height], [2, 1], 'actual preview shape');
			equal([...preview], expected, 'every independent preview RGBA');
			if (!(bytes[193] & (kind === 'dds' ? 64 : 8)))
				throw Error('Missing filters-baked receipt');
			if (bytes[193] & (kind === 'dds' ? 32 : 4))
				throw Error('Unrequested metadata omission');
		}
		return {
			mutation,
			status: bytes[10],
			encoded: [...encoded],
			preview: [...preview],
			encodedSha256: await digest(encoded),
			responseSha256: await digest(bytes),
			spatialContext: hex(bytes.subarray(416, 480)),
		};
	} finally {
		clearTimeout(timer);
		worker.terminate();
	}
}
