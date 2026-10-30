// Independent framing controls; actual Wasm/browser qualification is separate.
import assert from 'node:assert/strict';
import test from 'node:test';
import * as dds from '../src/lib/dds-worker-protocol.js';
import * as msp from '../src/lib/msp-worker-protocol.js';
import { context } from './test-support/source-metadata-context.mjs';

const owner = {
	session: '00000000-0000-4000-8000-000000000001',
	state: '00000000-0000-4000-8000-000000000002',
	epoch: 7n,
	frontend: 3,
};
const nonce = '00000000-0000-4000-8000-000000000003';
const text = new TextEncoder();
for (const [kind, api, family, flagAt, filterBit, metadataBit] of [
	['DDS', dds, 1, 121, 64, 32],
	['MSP', msp, 2, 114, 8, 4],
]) {
	const capture = api[`capture${kind === 'DDS' ? 'Dds' : 'Msp'}`];
	const freeze = api[`freeze${kind === 'DDS' ? 'Dds' : 'Msp'}Plan`];
	const decode = api[`decode${kind === 'DDS' ? 'Dds' : 'Msp'}Response`];
	const plan = (extra = {}) => ({
		width: 1,
		height: 1,
		sourceFlags: 4,
		filterDigest: new Uint8Array(32).fill(29),
		bakeIntent: 1,
		metadataContext: context(family),
		settings: new Uint8Array([0, 0, 0, 1, 0, 0, 0, 0]),
		version: 2,
		...extra,
	});
	async function frame(captured, { status = 0, mask = filterBit } = {}) {
		const out = new Uint8Array(384 + (status ? 0 : 5));
		const view = new DataView(out.buffer),
			h = captured.header;
		out.set(text.encode(`PC${kind}R03`));
		view.setUint16(8, 3);
		out[10] = status;
		out.set(h.subarray(16, 24), 12);
		out.set(h.subarray(12, 16), 20);
		view.setUint32(24, status ? 0 : 1);
		view.setUint16(28, status ? 0 : 1);
		view.setUint16(30, status ? 0 : 1);
		out.set(h.subarray(24, 65), 32);
		out.set(h.subarray(65, 81), 73);
		out.set(h.subarray(81, 113), 89);
		if (kind === 'DDS') out.set(h.subarray(113, 121), 121);
		else out[121] = 2;
		out[198] = 1;
		out[199] = 'x'.charCodeAt(0);
		out[265] = h[flagAt];
		out[266] = h[122];
		out.set(h.subarray(128, 160), 288);
		out.set(h.subarray(160, 224), 320);
		out[193] = status ? 0 : mask;
		if (!status) out.set([9, 0, 0, 0, 255], 384);
		out.set(
			new Uint8Array(
				await crypto.subtle.digest(
					'SHA-256',
					out.subarray(384, status ? 384 : 385),
				),
			),
			129,
		);
		out.set(
			new Uint8Array(
				await crypto.subtle.digest(
					'SHA-256',
					out.subarray(status ? 384 : 385),
				),
			),
			161,
		);
		return out;
	}
	test(`${kind} v3 binds independent bake intent and immutable inventory`, async () => {
		const p = plan();
		const pending = capture(new Uint8Array([1]), owner, p, nonce);
		p.filterDigest.fill(0);
		p.bakeIntent = 0;
		const captured = await pending;
		assert.equal(captured.header.length, 224);
		assert.equal(captured.header[122], 1);
		assert.deepEqual(
			captured.header.subarray(128, 160),
			new Uint8Array(32).fill(29),
		);
		const valid = await frame(captured);
		assert.equal(
			(await decode(valid, captured, owner, 'x')).lossMask,
			filterBit,
		);
		for (const at of [266, 267, 287, 288, 319, 320, 383]) {
			const bad = valid.slice();
			bad[at] ^= 1;
			await assert.rejects(decode(bad, captured, owner, 'x'), {
				code: 'CodecFailed',
			});
		}
		const old = valid.slice();
		old[7] = '2'.charCodeAt(0);
		new DataView(old.buffer).setUint16(8, 2);
		await assert.rejects(decode(old, captured, owner, 'x'), {
			code: 'CodecFailed',
		});
		for (const extra of [
			{ bakeIntent: 2 },
			{ bakeIntent: undefined },
			{ sourceFlags: 8 },
			{ filterDigest: new Uint8Array(31) },
		]) {
			assert.throws(() => freeze(plan(extra)), { code: 'InputSchema' });
		}
	});
	test(`${kind} inventory freeze never executes caller iterators`, async () => {
		const digest = new Uint8Array(32).fill(43);
		digest[Symbol.iterator] = () => {
			throw Error('Caller iterator must not run');
		};
		const captured = await capture(
			new Uint8Array([1]),
			owner,
			plan({ filterDigest: digest }),
			nonce,
		);
		assert.deepEqual(
			captured.header.subarray(128, 160),
			new Uint8Array(32).fill(43),
		);
	});
	test(`${kind} metadata omission and preview loss masks never grant Bake`, async () => {
		const captured = await capture(
			new Uint8Array([1]),
			owner,
			plan({
				bakeIntent: 0,
				metadataContext: context(family, { present: true, omit: true }),
			}),
			nonce,
		);
		await assert.rejects(
			decode(
				await frame(captured, { mask: filterBit | metadataBit }),
				captured,
				owner,
				'x',
			),
			{ code: 'CodecFailed' },
		);
		await assert.rejects(
			decode(await frame(captured, { status: 1 }), captured, owner, 'x'),
			{ code: 'CapabilityUnsupported' },
		);
		const baked = await capture(
			new Uint8Array([1]),
			owner,
			plan({
				metadataContext: context(family, { present: true, omit: true }),
			}),
			nonce,
		);
		assert.equal(
			(
				await decode(
					await frame(baked, { mask: filterBit | metadataBit }),
					baked,
					owner,
					'x',
				)
			).lossMask,
			filterBit | metadataBit,
		);
	});
}
