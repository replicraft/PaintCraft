// Protocol-v2 source facts. This is NOT host publication/omission authority.
import { refusal } from './codec-worker-protocol.js';
export const SOURCE_METADATA_CONTEXT_BYTES = 64;
export function freezeMetadataContext(bytes, target) {
	if (!(bytes instanceof Uint8Array) || bytes.length !== 64) {
		throw refusal('InputSchema', 'Invalid source metadata context');
	}
	const copy = bytes.slice();
	const view = new DataView(copy.buffer);
	const flags = copy[48];
	const wire = view.getUint32(52);
	const charge = view.getBigUint64(56);
	const present = Boolean(flags & 1);
	if (
		flags > 3 ||
		copy[49] !== target ||
		copy[50] !== 1 ||
		copy[51] ||
		(!present && (copy.subarray(16, 48).some(Boolean) || wire || charge)) ||
		(present &&
			(!(flags & 2) ||
				wire < 192 ||
				wire > 8388608 ||
				charge < BigInt(wire) ||
				charge > 8454144n))
	) {
		throw refusal(
			'InputSchema',
			'Invalid source metadata intent or bounds',
		);
	}
	return Object.freeze([...copy]);
}
export function metadataOmitted(context) {
	return (context[48] & 3) === 3;
}
