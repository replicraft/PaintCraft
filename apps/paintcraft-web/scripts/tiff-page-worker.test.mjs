// Independent framing/ownership oracle; synthetic payloads never claim TIFF decode.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';
import {
	captureTiff,
	decodeTiffResponse,
	encodeTiffRequest,
	freezeTiffPlan,
	isCompletedTiffRefusal,
	TIFF_REPORT_BYTES,
	TIFF_REQUEST_HEADER,
	TIFF_RESPONSE_HEADER,
	TIFF_ROW_BYTES,
	TIFF_SIDE_MAX,
} from '../src/lib/tiff-page-worker-protocol.js';

const owner = {
	session: '00000000-0000-0000-0000-000000000001',
	state: '00000000-0000-0000-0000-000000000002',
	epoch: 3n,
	frontend: 3,
};
const nonce = '00000000-0000-0000-0000-000000000004';
const sha = (bytes) =>
	new Uint8Array(createHash('sha256').update(bytes).digest());
const hex = (bytes) => Buffer.from(sha(bytes)).toString('hex');
function row(index, supported, width, height, offset) {
	const out = new Uint8Array(TIFF_ROW_BYTES),
		view = new DataView(out.buffer);
	view.setUint16(0, index);
	out[2] = Number(supported);
	view.setUint32(4, width);
	view.setUint32(8, height);
	view.setBigUint64(12, BigInt(offset));
	return out;
}
function response(
	captured,
	{
		status = 0,
		pages = 2,
		width = 0,
		height = 0,
		report = new Uint8Array(),
		payload = new Uint8Array([
			...row(0, false, 0, 0, 8),
			...row(1, true, 1, 1, 32),
		]),
	} = {},
) {
	const version = new TextEncoder().encode('0.1.1');
	const out = new Uint8Array(
			TIFF_RESPONSE_HEADER +
				version.length +
				report.length +
				payload.length,
		),
		view = new DataView(out.buffer);
	out.set(new TextEncoder().encode('PCTIR001'));
	view.setUint16(8, 1);
	out[10] = captured.header[10];
	out[11] = status;
	out.set(captured.header.subarray(12, 14), 12);
	view.setUint16(14, pages);
	view.setUint32(16, width);
	view.setUint32(20, height);
	view.setBigUint64(24, BigInt(payload.length));
	view.setUint32(32, report.length);
	view.setUint16(36, version.length);
	out.set(captured.header.subarray(24, 65), 40);
	out.set(captured.header.subarray(65, 81), 81);
	out.set(captured.header.subarray(81, 113), 97);
	out.set(sha(payload), 129);
	out.set(sha(report), 161);
	out.set(version, TIFF_RESPONSE_HEADER);
	out.set(report, TIFF_RESPONSE_HEADER + version.length);
	out.set(payload, TIFF_RESPONSE_HEADER + version.length + report.length);
	return out;
}
async function request(selected = false) {
	const bytes = new Uint8Array(64).fill(31);
	const plan = selected
		? {
				index: 1,
				pageCount: 2,
				width: 1,
				height: 1,
				ifdOffset: 32,
				sourceSha256: hex(bytes),
			}
		: null;
	return captureTiff(bytes, owner, plan, nonce);
}
test('TIFF inventory and selection use a distinct whole-source request, not decoder fields', async () => {
	for (const selected of [false, true]) {
		const captured = await request(selected),
			bytes = encodeTiffRequest(captured),
			view = new DataView(bytes.buffer);
		assert.equal(
			new TextDecoder().decode(bytes.subarray(0, 8)),
			'PCTIF001',
		);
		assert.equal(bytes[10], selected ? 2 : 1);
		assert.equal(view.getUint16(12), selected ? 1 : 65535);
		assert.equal(view.getBigUint64(16), 64n);
		assert.equal(bytes.length, TIFF_REQUEST_HEADER + 64);
		assert.deepEqual(
			bytes.subarray(TIFF_REQUEST_HEADER),
			new Uint8Array(64).fill(31),
		);
		assert.deepEqual(
			bytes.subarray(81, 113),
			sha(bytes.subarray(TIFF_REQUEST_HEADER)),
		);
		assert.equal(view.getBigUint64(159), selected ? 32n : 0n);
	}
});
test('selection is explicit and frozen to the displayed source hash and absolute IFD', async () => {
	const bytes = new Uint8Array(64).fill(31),
		original = bytes.slice();
	const promise = captureTiff(bytes, owner, null, nonce);
	bytes.fill(255);
	const captured = await promise;
	assert.deepEqual(captured.source, original);
	const plan = {
		index: 1,
		pageCount: 2,
		width: 1,
		height: 1,
		ifdOffset: 32,
		sourceSha256: hex(original),
	};
	assert.equal(Object.isFrozen(freezeTiffPlan(plan)), true);
	await assert.rejects(captureTiff(bytes, owner, plan, nonce), {
		code: 'ConflictStaleBase',
	});
	for (const changed of [
		{ index: undefined },
		{ index: -1 },
		{ index: 2 },
		{ pageCount: 1025 },
		{ ifdOffset: 0 },
		{ width: 0 },
		{ sourceSha256: 'bad' },
	])
		assert.throws(() => freezeTiffPlan({ ...plan, ...changed }));
});
test('unsupported first TIFF page remains inventoried and later qualified page is retained', async () => {
	const captured = await request(),
		frame = await decodeTiffResponse(
			response(captured),
			captured,
			owner,
			'0.1.1',
		);
	assert.equal(frame.pageCount, 2);
	assert.equal(frame.payload[2], 0);
	assert.equal(frame.payload[TIFF_ROW_BYTES + 2], 1);
	assert.equal(
		new DataView(
			frame.payload.buffer,
			frame.payload.byteOffset,
		).getBigUint64(TIFF_ROW_BYTES + 12),
		32n,
	);
});
test('every inventory and selected response byte is bound before Rust receives the frame', async () => {
	for (const selected of [false, true]) {
		const captured = await request(selected);
		const bytes = selected
			? response(captured, {
					width: 1,
					height: 1,
					report: new Uint8Array(TIFF_REPORT_BYTES),
					payload: new Uint8Array([17, 29, 91, 0]),
				})
			: response(captured);
		await decodeTiffResponse(bytes, captured, owner, '0.1.1');
		for (let at = 0; at < bytes.length; at++) {
			const changed = bytes.slice();
			changed[at] ^= 1;
			await assert.rejects(
				decodeTiffResponse(changed, captured, owner, '0.1.1'),
				undefined,
				`unbound byte ${at}`,
			);
		}
		await assert.rejects(
			decodeTiffResponse(
				bytes,
				captured,
				{ ...owner, epoch: 4n },
				'0.1.1',
			),
		);
		await assert.rejects(
			decodeTiffResponse(bytes, captured, owner, 'other-version'),
		);
		await assert.rejects(
			decodeTiffResponse(bytes.subarray(0, -1), captured, owner, '0.1.1'),
		);
	}
});
test('only exact completed refusal releases the operation proof; arbitrary errors do not', async () => {
	const captured = await request(true),
		other = await request(true);
	const bytes = response(captured, {
		status: 2,
		pages: 0,
		payload: new Uint8Array(),
	});
	let error;
	try {
		await decodeTiffResponse(bytes, captured, owner, '0.1.1');
	} catch (value) {
		error = value;
	}
	assert.equal(error.code, 'CapabilityUnsupported');
	assert.equal(isCompletedTiffRefusal(error, captured), true);
	assert.equal(isCompletedTiffRefusal(error, other), false);
	assert.equal(
		isCompletedTiffRefusal(
			{ code: error.code, codecCompleted: true },
			captured,
		),
		false,
	);
	const corrupt = bytes.slice();
	corrupt[97] ^= 1;
	await assert.rejects(
		decodeTiffResponse(corrupt, captured, owner, '0.1.1'),
		(value) => !isCompletedTiffRefusal(value, captured),
	);
});

test('narrow TIFF pages above 65536 use the canonical side bound within 4MP', async () => {
	assert.equal(TIFF_SIDE_MAX, 1_048_576);
	const source = new Uint8Array(64).fill(31);
	for (const [width, height] of [
		[65537, 1],
		[1, 65537],
		[TIFF_SIDE_MAX, 1],
		[1, TIFF_SIDE_MAX],
	]) {
		const plan = {
			index: 1,
			pageCount: 2,
			width,
			height,
			ifdOffset: 32,
			sourceSha256: hex(source),
		};
		assert.equal(freezeTiffPlan(plan).width, width);
		const captured = await captureTiff(source, owner, null, nonce);
		const payload = new Uint8Array([
			...row(0, false, 0, 0, 8),
			...row(1, true, width, height, 32),
		]);
		assert.equal(
			(
				await decodeTiffResponse(
					response(captured, { payload }),
					captured,
					owner,
					'0.1.1',
				)
			).pageCount,
			2,
		);
	}
	for (const [width, height] of [
		[TIFF_SIDE_MAX + 1, 1],
		[1, TIFF_SIDE_MAX + 1],
		[TIFF_SIDE_MAX, 5],
	])
		assert.throws(() =>
			freezeTiffPlan({
				index: 1,
				pageCount: 2,
				width,
				height,
				ifdOffset: 32,
				sourceSha256: hex(source),
			}),
		);
});
