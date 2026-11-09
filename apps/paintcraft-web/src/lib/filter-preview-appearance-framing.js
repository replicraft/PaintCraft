// Additive v3 frames. Source coverage/inventory are opaque Rust-authenticated SHA.
import { requirementDigests } from './filter-preview-program-protocol.js';
import {
	APPEARANCE7,
	decimal7,
	fail7,
	freeze7,
	numericalEntry7,
	sha7,
	shape7,
	snapshot7,
	validateAdmission7,
	validateFacts7,
	validatePairs7,
	validateUnion7,
	equal7,
} from './filter-preview-appearance-common.js';
import { canonicalOperation7 } from './filter-preview-appearance-operation.js';
const encoder = new TextEncoder();
class Frame {
	constructor(domain) {
		this.values = [...encoder.encode(domain)];
	}
	push(bytes) {
		if (this.values.length + bytes.length > 4096)
			throw fail7('ResourceLimit', 'Source7 fixed frame');
		for (const b of bytes) this.values.push(b);
	}
	number(value, width) {
		let v = BigInt(value);
		const out = new Uint8Array(width);
		for (let i = width - 1; i >= 0; i--) {
			out[i] = Number(v & 255n);
			v >>= 8n;
		}
		if (v !== 0n) throw fail7('InputSchema', 'Source7 frame integer');
		this.push(out);
	}
	text(value) {
		const bytes = encoder.encode(value);
		this.number(bytes.length, 2);
		this.push(bytes);
	}
	hash(value) {
		sha7(value);
		const bytes = new Uint8Array(32);
		for (let i = 0; i < 32; i++)
			bytes[i] = Number.parseInt(value.slice(i * 2, i * 2 + 2), 16);
		this.push(bytes);
	}
	async digest() {
		const bytes = Uint8Array.from(this.values);
		return [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
			.map((b) => b.toString(16).padStart(2, '0'))
			.join('');
	}
}
export async function admissionDigest7(value) {
	const a = snapshot7(value);
	validateAdmission7(a);
	const b = new Frame('paintcraft.source7-admission/v2\0');
	b.number(a.mode === 'base' ? 0 : 1, 1);
	b.number(a.workAuthority === 'captured-job' ? 0 : 1, 1);
	for (const key of [
		'workspaceBytes',
		'baseWorkLimit',
		'noiseWorkLimit',
		'totalWorkLimit',
	])
		b.number(decimal7(a[key]), 8);
	b.number(1, 2);
	b.number(64, 2);
	b.number(8388608, 8);
	b.number(a.mode === 'noise' ? 134217728 : 0, 8);
	return b.digest();
}
async function prefix(domain, fact, admission) {
	const b = new Frame(domain);
	b.number(3, 2);
	b.hash(fact.requiredSha256);
	b.hash(fact.resolutionSha256);
	b.hash(await admissionDigest7(admission));
	b.text(APPEARANCE7.feature_id);
	b.text(APPEARANCE7.resource_contract_id);
	b.number(APPEARANCE7.resource_contract_revision, 2);
	b.number(APPEARANCE7.query_revision, 2);
	b.number(APPEARANCE7.alpha_revision, 2);
	b.text(APPEARANCE7.workload_class_id);
	b.number(APPEARANCE7.workload_class_revision, 2);
	return b;
}
export async function capabilityDigest7(value, admission) {
	const f = snapshot7(value),
		a = snapshot7(admission);
	validateFacts7(f);
	validateAdmission7(a);
	const b = await prefix('paintcraft.worker-capabilities/v3\0', f, a),
		own = f.layerMasks;
	b.number(own.stored, 4);
	b.number(own.enabled, 4);
	b.number(own.bands, 8);
	b.number(own.runs, 8);
	b.number(decimal7(own.decodeUpperBytes), 8);
	b.hash(own.identitySha256);
	b.hash(f.sourceCoverageSha256);
	b.hash(f.maskInventorySha256);
	return b.digest();
}
export async function unionDigest7(value, admission) {
	const f = snapshot7(value),
		a = snapshot7(admission);
	validateUnion7(f);
	validateAdmission7(a);
	const b = await prefix('paintcraft.worker-capability-union/v1\0', f, a);
	b.hash(f.sourceCoverageSha256);
	return b.digest();
}
export async function verifyFacts7(value, admission) {
	const f = snapshot7(value),
		a = snapshot7(admission);
	validateFacts7(f);
	if ((await capabilityDigest7(f, a)) !== f.capabilitySha256)
		throw fail7('CodecFailed', 'Source7 capability binding');
	return freeze7(f);
}
export async function verifyUnion7(value, admission) {
	const f = snapshot7(value),
		a = snapshot7(admission);
	validateUnion7(f);
	if ((await unionDigest7(f, a)) !== f.capabilitySha256)
		throw fail7('CodecFailed', 'Source7 union binding');
	return freeze7(f);
}
export async function numericalDigests7(pairValue, entryValue) {
	const pairs = snapshot7(pairValue),
		entries = snapshot7(entryValue);
	validatePairs7(pairs);
	if (!Array.isArray(entries) || entries.length !== pairs.length)
		throw fail7('CodecFailed', 'Source7 exact requested entries');
	for (let i = 0; i < pairs.length; i++) {
		shape7(entries[i], [
			'profile_id',
			'codec_id',
			'resource_contract_id',
			'resource_contract_revision',
			'workload_class_id',
			'workload_class_revision',
		]);
		if (!equal7(entries[i], numericalEntry7(pairs[i])))
			throw fail7(
				'CapabilityUnsupported',
				'Source7 registration resource/class',
			);
	}
	// Validate closed shapes/ID bounds BEFORE awaiting digest work.
	const base = entries.map(
		({
			profile_id,
			codec_id,
			resource_contract_id,
			resource_contract_revision,
		}) => ({
			profile_id,
			codec_id,
			resource_contract_id,
			resource_contract_revision,
		}),
	);
	return requirementDigests(pairs, base);
}
export async function operationDigest7(operation) {
	const text = canonicalOperation7(operation);
	const digest = await crypto.subtle.digest('SHA-256', encoder.encode(text));
	return [...new Uint8Array(digest)]
		.map((b) => b.toString(16).padStart(2, '0'))
		.join('');
}
