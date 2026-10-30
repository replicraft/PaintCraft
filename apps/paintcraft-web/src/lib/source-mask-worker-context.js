// Protocol4 resource facts, never filter-bake or publication authority.
import { refusal } from './codec-worker-protocol.js';
export const SOURCE_MASK_CONTEXT_BYTES = 32;
export function freezeMaskContext(bytes, sourceFlags) {
	if (!(bytes instanceof Uint8Array) || bytes.length !== 32)
		throw refusal('InputSchema', 'Invalid source mask context');
	// Fixed indexing: user-provided iteration cannot replace the captured facts.
	const copy = Uint8Array.from({ length: 32 }, (_, index) => bytes[index]);
	const view = new DataView(copy.buffer);
	const [masks, bands, runs, charge] = [0, 8, 16, 24].map((at) =>
		view.getBigUint64(at, true),
	);
	if (
		masks > 2048n ||
		bands > 262144n ||
		runs > 262144n ||
		charge > 268435456n ||
		bands > runs ||
		(bands === 0n) !== (runs === 0n) ||
		(masks === 0n && (bands !== 0n || runs !== 0n || charge !== 0n)) ||
		(bands === 0n && charge !== 0n) ||
		(bands > 0n && charge === 0n) ||
		Boolean(sourceFlags & 8) !== masks > 0n ||
		(masks > 0n && !(sourceFlags & 4))
	)
		throw refusal('InputSchema', 'Invalid expanded mask declaration');
	return Object.freeze(Array.from({ length: 32 }, (_, index) => copy[index]));
}
