// Separate Export8 descriptive contexts. Rust rederives all authority before decode.
import { refusal, equal } from './codec-worker-protocol.js';
const fail = (m) => {
	throw refusal('InputSchema', m);
};
function bytes(value, length) {
	if (!(value instanceof Uint8Array) || value.length !== length)
		fail('DDS8 context extent');
	const result = new Uint8Array(length);
	for (let i = 0; i < length; i++) result[i] = value[i];
	return result;
}
function frozen(value) {
	return Object.freeze(
		Array.from({ length: value.length }, (_, i) => value[i]),
	);
}
export function freezeNoiseWorkload(value) {
	const b = bytes(value, 64),
		v = new DataView(b.buffer),
		n = (i) => v.getBigUint64(i);
	if (
		n(0) > 67108864n ||
		n(8) > 268435456n ||
		n(16) > 34359738368n ||
		n(24) !== n(8) + n(16)
	)
		fail('DDS8 workload limits');
	return frozen(b);
}
export async function verifyNoiseWorkload(value, crypto = globalThis.crypto) {
	const b = bytes(value, 64),
		id = new TextEncoder().encode('noise-source-work-v1'),
		domain = new TextEncoder().encode('paintcraft.noise.workload/v1\0');
	const all = new Uint8Array(domain.length + 2 + id.length + 4 + 32);
	all.set(domain);
	let at = domain.length;
	const v = new DataView(all.buffer);
	v.setUint16(at, id.length);
	at += 2;
	all.set(id, at);
	at += id.length;
	v.setUint32(at, 1);
	at += 4;
	all.set(b.subarray(0, 32), at);
	if (
		!equal(
			new Uint8Array(await crypto.subtle.digest('SHA-256', all)),
			b.subarray(32),
		)
	)
		fail('DDS8 workload hash');
}
export function freezeNoiseContexts(region, program, workload, cap) {
	const r = bytes(region, 128),
		p = bytes(program, 256),
		w = bytes(workload, 64),
		rv = new DataView(r.buffer),
		pv = new DataView(p.buffer),
		wv = new DataView(w.buffer),
		rn = (i) => rv.getBigUint64(i),
		pn = (i) => pv.getBigUint64(i);
	freezeNoiseWorkload(w);
	for (const b of [r, p]) {
		const v = new DataView(b.buffer);
		if (
			v.getUint16(0) !== 8 ||
			v.getUint16(2) !== 1 ||
			v.getUint16(4) !== 6 ||
			v.getUint16(6) !== 2
		)
			fail('DDS8 context version');
	}
	if (
		r.subarray(112).some(Boolean) ||
		p.subarray(224).some(Boolean) ||
		pv.getUint32(20) > 256 ||
		pv.getUint32(8) > 256 ||
		pv.getUint32(12) > 2048 ||
		pv.getUint32(16) > pv.getUint32(12) ||
		pv.getUint32(8) > pv.getUint32(12) ||
		pv.getUint32(20) > pv.getUint32(16)
	)
		fail('DDS8 context inventory/reserved');
	if (
		!rv.getUint32(16) ||
		rv.getUint32(16) + rv.getUint32(8) > 2048 ||
		rv.getUint32(20) > rv.getUint32(16) ||
		rv.getUint32(12) > rv.getUint32(8)
	)
		fail('DDS8 stored Noise');
	if (
		rn(56) !== rn(40) + rn(48) ||
		pn(72) !== pn(56) + pn(64) ||
		pn(56) !== rn(40) + pn(24) ||
		pn(64) !== rn(48) ||
		rn(32) > wv.getBigUint64(0) ||
		rn(72) === 0n ||
		rn(72) > 33554432n ||
		rn(64) < rn(32) + rn(72) ||
		rn(64) > BigInt(cap)
	)
		fail('DDS8 context arithmetic');
	if (
		pn(48) !== pn(32) + rn(32) + pn(80) ||
		pn(40) < pn(32) ||
		pn(40) + pn(80) > wv.getBigUint64(0) ||
		pn(48) > wv.getBigUint64(0) ||
		pn(56) > wv.getBigUint64(8) ||
		pn(64) > wv.getBigUint64(16) ||
		pv.getBigUint64(88) === 0n ||
		!equal(r.subarray(80, 112), w.subarray(32)) ||
		!equal(p.subarray(160, 192), w.subarray(32))
	)
		fail('DDS8 context ceilings/binding');
	const peak = pn(48) > pn(40) + pn(80) ? pn(48) : pn(40) + pn(80);
	if (rn(64) - rn(32) + peak + pn(88) > BigInt(cap))
		fail('DDS8 aggregate phase cap');
	return { region: frozen(r), program: frozen(p), workload: frozen(w) };
}
