// Private protocol5 accounting, not bake/privacy/publication authority.
import { refusal } from './codec-worker-protocol.js';
export function freezeSpatialContext(bytes, cap) {
	if (!(bytes instanceof Uint8Array) || bytes.length !== 64)
		throw refusal('InputSchema', 'Invalid spatial export context');
	const copy = new Uint8Array(64);
	for (let i = 0; i < 64; i++) copy[i] = bytes[i];
	const v = new DataView(copy.buffer),
		n = (at) => v.getBigUint64(at);
	if (
		v.getUint16(0) !== 5 ||
		v.getUint16(2) !== 1 ||
		!v.getUint32(4) ||
		v.getUint32(4) > 2048 ||
		v.getUint32(8) > v.getUint32(4) ||
		copy.subarray(12, 16).some((b) => b !== 0) ||
		copy.subarray(48).some((b) => b !== 0) ||
		(!v.getUint32(8) && n(24) !== 0n) ||
		n(32) > 268435456n ||
		n(40) < n(24) ||
		n(40) > BigInt(cap)
	)
		throw refusal(
			'ResourceLimit',
			'Spatial export accounting exceeds its profile',
		);
	return Object.freeze(Array.from({ length: 64 }, (_, i) => copy[i]));
}
