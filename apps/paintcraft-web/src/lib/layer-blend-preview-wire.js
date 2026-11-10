// Exact fixed-window adapter. No pointer owns Rust capacity or authorization.
import {
	VERSIONS8,
	LIMITS8,
	ROLES8,
	decimal8,
	uint8,
	sha8,
	shape8,
	fail8,
} from './layer-blend-preview-common.js';
import {
	validateReady8,
	validateOpen8,
	geometry8,
} from './layer-blend-preview-protocol.js';
export const CONTROL_JS8 = 8 * 4096 + 12 * 2048 + 512;
export const STATUS8 = Object.freeze({
	filling: 0,
	opened: 1,
	viewing: 2,
	patch: 3,
	complete: 4,
	cancelled: 5,
	closed: 6,
});
const runtimeKeys = [...Object.keys(VERSIONS8), ...Object.keys(LIMITS8)];
const controlKeys = ['serviceBytes', 'stageBytes', 'viewBytes'];
export function getterU648(value) {
	if (
		typeof value !== 'bigint' ||
		value < -0x8000000000000000n ||
		value > 0xffffffffffffffffn
	)
		fail8('CodecFailed', 'Source8 actual unsigned64 getter');
	return BigInt.asUintN(64, value);
}
export function compiledFacts8(api) {
	const facts = { ...VERSIONS8, limits: {}, controls: {} };
	for (const [i, key] of runtimeKeys.entries()) {
		const n = getterU648(api.layer_blend_runtime_fact(i));
		if (Object.hasOwn(VERSIONS8, key)) facts[key] = Number(n);
		else
			facts.limits[key] =
				typeof LIMITS8[key] === 'string' ? n.toString() : Number(n);
	}
	for (const [i, key] of controlKeys.entries())
		facts.controls[key] = getterU648(
			api.layer_blend_control_fact(i),
		).toString();
	return validateReady8({ kind: 'ready', runtimeFacts: facts }, facts);
}
export function hBytes8(facts) {
	const h = controlKeys.reduce(
		(n, key) => n + decimal8(facts.controls[key], 1n),
		0n,
	);
	if (h > BigInt(LIMITS8.jobBytes))
		fail8('ResourceLimit', 'Source8 compiled H sum');
	return Number(h);
}
export async function profile8(facts, crypto = globalThis.crypto) {
	const prefix = new TextEncoder().encode(
		'paintcraft.worker-source8-service/v1\0',
	);
	const bytes = new Uint8Array(prefix.length + 11 * 4 + 11 * 8),
		d = new DataView(bytes.buffer);
	bytes.set(prefix);
	let at = prefix.length;
	for (const key of Object.keys(VERSIONS8)) {
		d.setUint32(at, facts[key], true);
		at += 4;
	}
	for (const key of Object.keys(LIMITS8)) {
		d.setBigUint64(at, BigInt(facts.limits[key]), true);
		at += 8;
	}
	for (const key of controlKeys) {
		d.setBigUint64(at, decimal8(facts.controls[key]), true);
		at += 8;
	}
	return Array.from(
		new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)),
		(n) => n.toString(16).padStart(2, '0'),
	).join('');
}
export function getterProfile8(api) {
	const bytes = new Uint8Array(32),
		d = new DataView(bytes.buffer);
	for (let i = 0; i < 8; i++)
		d.setUint32(i * 4, uint8(api.layer_blend_profile_word(i)), true);
	return Array.from(bytes, (n) => n.toString(16).padStart(2, '0')).join('');
}
const hex = (text) =>
	Uint8Array.from(text.match(/../g), (s) => Number.parseInt(s, 16));
export function window8(kind, request, open, operation) {
	const kinds = { open: 1, view: 2, ack: 3, cancel: 4, close: 5 };
	if (!Object.hasOwn(kinds, kind))
		fail8('InputSchema', 'Source8 binary kind');
	uint8(request, 0xffffffff, 1);
	if (open) validateOpen8(open);
	if (operation && kind !== 'close') {
		geometry8(operation.visible, operation.level, open.canvas);
		if (kind === 'ack') uint8(operation.ordinal);
	}
	const bytes = new Uint8Array(512),
		d = new DataView(bytes.buffer);
	bytes.set([80, 67, 56, 87]);
	d.setUint16(4, 1, true);
	d.setUint8(6, kinds[kind]);
	d.setUint32(8, request, true);
	if (open) {
		d.setUint8(
			7,
			(open.mode === 'noise' ? 1 : 0) | (open.storedNoise ? 2 : 0) | 4,
		);
		for (const [offset, id] of [
			[16, open.owner],
			[32, open.publicTicket.state],
			[56, open.document],
			[72, open.sourceState],
			[88, open.draft],
		])
			bytes.set(hex(id.replaceAll('-', '')), offset);
		d.setBigUint64(48, decimal8(open.publicTicket.epoch), true);
		bytes.set(hex(open.sourceSha256), 104);
		bytes.set(hex(open.profileSha256), 136);
		d.setUint32(168, open.canvas[0], true);
		d.setUint32(172, open.canvas[1], true);
		for (const [offset, key] of [
			[176, 'byteLength'],
			[184, 'baseWork'],
			[192, 'noiseWork'],
			[200, 'pairedBytes'],
		])
			d.setBigUint64(offset, decimal8(open[key]), true);
		for (const [i, key] of ROLES8.entries())
			d.setBigUint64(208 + i * 8, decimal8(open.roles[key]), true);
	}
	if (operation && kind !== 'close') {
		d.setBigUint64(304, decimal8(operation.token, 1n), true);
		d.setBigUint64(312, decimal8(operation.generation, 1n), true);
		for (const [i, n] of operation.visible.entries())
			d.setUint32(320 + i * 4, n, true);
		d.setUint32(336, operation.level, true);
		d.setUint32(340, kind === 'ack' ? operation.ordinal : 0, true);
	}
	return bytes;
}
export function pointer8(memory, pointer, length, expected) {
	uint8(pointer);
	uint8(length);
	if (length !== expected || pointer + length > memory.buffer.byteLength)
		fail8('CodecFailed', 'Source8 exact borrowed memory extent');
}
export function writeWindow8(service, memory, bytes) {
	const pointer = service.controlPtr(),
		length = service.controlLen();
	pointer8(memory, pointer, length, 512);
	// No Rust calls while this mutable view exists; all pointer queries precede it.
	new Uint8Array(memory.buffer, pointer, length).set(bytes);
}
const CODES = new Set([
	'input.schema',
	'policy.unknown',
	'reference.unknown_alias',
	'reference.unknown_id',
	'target.invalid',
	'capability.unsupported',
	'conflict.stale_base',
	'resource.limit',
	'operation.cancelled',
	'codec.failed',
	'storage.failed',
	'invariant.violation',
]);
export function typedRustError8(value) {
	if (typeof value !== 'string' || value.length > 8192) return null;
	try {
		const v = JSON.parse(value);
		shape8(v, ['code', 'message', 'operationIndex']);
		if (
			!CODES.has(v.code) ||
			typeof v.message !== 'string' ||
			v.message.length > 4096
		)
			return null;
		if (v.operationIndex !== null) uint8(v.operationIndex);
		return v;
	} catch {
		return null;
	}
}
export function verifyOpenCompiled8(open, facts, profile) {
	sha8(profile);
	if (
		open.profileSha256 !== profile ||
		decimal8(open.roles.H) !== BigInt(hBytes8(facts)) ||
		(open.mode === 'base' && open.pairedBytes !== '0') ||
		decimal8(open.roles.Q) % 4n !== 0n
	)
		fail8('CapabilityUnsupported', 'Source8 actual compiled profile/roles');
}
export function expectStatus8(actual, expected) {
	if (actual !== expected)
		fail8('CodecFailed', 'Source8 genuine service status');
}
