// Protocol8 metadata validation only: no execution, allocation or credit authority.
export const LIMITS8 = Object.freeze({
	jobBytes: '67108864',
	encodedBytes: '67108864',
	baseWork: '268435456',
	noiseWork: '34359738368',
	payloadBytes: '234881024',
	linearBytes: '268435456',
	outputCells: 4194304,
	patchEdge: 64,
});
export const VERSIONS8 = Object.freeze({
	protocol: 8,
	registryContract: 4,
	sourceFormat: 8,
	appearanceResource: 2,
	blendResource: 1,
	arithmeticRevision: 1,
	workRevision: 1,
	queryRevision: 1,
	sliceRevision: 1,
	nativeEdge: 64,
	displayRevision: 1,
});
export const ROLES8 = Object.freeze([
	'E',
	'P',
	'M',
	'A',
	'I',
	'C',
	'Q',
	'K',
	'W',
	'F',
	'O',
	'H',
]);
export const IDENTITY8 = Object.freeze([
	'owner',
	'publicTicket',
	'document',
	'sourceState',
	'draft',
	'sourceSha256',
	'profileSha256',
]);
export function fail8(code, message) {
	throw Object.assign(new Error(message), { code });
}
// Reflect own keys, not just enumerable JSON keys. Never invoke accessors.
export function shape8(value, keys) {
	if (
		!value ||
		typeof value !== 'object' ||
		Object.getPrototypeOf(value) !== Object.prototype
	)
		fail8('InputSchema', 'Protocol8 plain record');
	const own = Reflect.ownKeys(value);
	if (
		own.length !== keys.length ||
		own.some((k) => typeof k !== 'string' || !keys.includes(k))
	)
		fail8('InputSchema', 'Protocol8 exact own fields');
	for (const key of keys) {
		const d = Object.getOwnPropertyDescriptor(value, key);
		if (!d || !Object.hasOwn(d, 'value') || !d.enumerable)
			fail8('InputSchema', 'Protocol8 enumerable data fields');
	}
}
export function uint8(n, max = 0xffffffff, min = 0) {
	if (!Number.isSafeInteger(n) || Object.is(n, -0) || n < min || n > max)
		fail8('InputSchema', 'Protocol8 unsigned integer');
	return n;
}
export function decimal8(s, min = 0n) {
	if (typeof s !== 'string' || !/^(0|[1-9][0-9]{0,19})$/.test(s))
		fail8('InputSchema', 'Protocol8 canonical u64');
	const n = BigInt(s);
	if (n < min || n > 0xffffffffffffffffn)
		fail8('InputSchema', 'Protocol8 u64 bound');
	return n;
}
export function uuid8(s) {
	if (
		typeof s !== 'string' ||
		!/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/.test(s)
	)
		fail8('InputSchema', 'Protocol8 UUID');
}
export function sha8(s) {
	if (typeof s !== 'string' || !/^[0-9a-f]{64}$/.test(s))
		fail8('InputSchema', 'Protocol8 SHA256');
}
export function pair8(v) {
	if (
		!Array.isArray(v) ||
		Object.getPrototypeOf(v) !== Array.prototype ||
		v.length !== 2 ||
		Reflect.ownKeys(v).length !== 3
	)
		fail8('InputSchema', 'Protocol8 canvas pair');
	for (const k of ['0', '1']) {
		const d = Object.getOwnPropertyDescriptor(v, k);
		if (!d || !Object.hasOwn(d, 'value') || !d.enumerable)
			fail8('InputSchema', 'Protocol8 data pair');
		uint8(d.value, 1048576, 1);
	}
}
export function rect8(v, size) {
	if (
		!Array.isArray(v) ||
		Object.getPrototypeOf(v) !== Array.prototype ||
		v.length !== 4 ||
		Reflect.ownKeys(v).length !== 5
	)
		fail8('InputSchema', 'Protocol8 rectangle');
	for (const k of ['0', '1', '2', '3']) {
		const d = Object.getOwnPropertyDescriptor(v, k);
		if (!d || !Object.hasOwn(d, 'value') || !d.enumerable)
			fail8('InputSchema', 'Protocol8 data rectangle');
		uint8(d.value, 1048576);
	}
	if (v[2] <= v[0] || v[3] <= v[1] || v[2] > size[0] || v[3] > size[1])
		fail8('InputSchema', 'Protocol8 rectangle bounds');
}
export function identity8(v) {
	for (const key of ['owner', 'document', 'sourceState', 'draft'])
		uuid8(v[key]);
	for (const key of ['sourceSha256', 'profileSha256']) sha8(v[key]);
	shape8(v.publicTicket, ['state', 'epoch']);
	uuid8(v.publicTicket.state);
	decimal8(v.publicTicket.epoch);
}
export function equal8(a, b) {
	if (a === b) return true;
	if (!a || !b || typeof a !== 'object' || typeof b !== 'object')
		return false;
	const ak = Object.keys(a),
		bk = Object.keys(b);
	return (
		ak.length === bk.length &&
		ak.every((k) => Object.hasOwn(b, k) && equal8(a[k], b[k]))
	);
}
export function bind8(v, expected, keys = IDENTITY8) {
	for (const key of keys)
		if (!equal8(v[key], expected[key]))
			fail8('ConflictStaleBase', `Protocol8 foreign ${key}`);
}
export function frozen8(v) {
	if (v && typeof v === 'object') {
		for (const child of Object.values(v)) frozen8(child);
		Object.freeze(v);
	}
	return v;
}
export function copy8(v) {
	// Call only AFTER closed shape/type validation. No caller accessor survives.
	return frozen8(JSON.parse(JSON.stringify(v)));
}
export function buffer8(buffer, bytes) {
	if (
		!(buffer instanceof ArrayBuffer) ||
		Object.getPrototypeOf(buffer) !== ArrayBuffer.prototype ||
		Reflect.ownKeys(buffer).length !== 0 ||
		buffer.resizable ||
		buffer.byteLength !== bytes
	)
		fail8('InputSchema', 'Protocol8 exact fixed ArrayBuffer');
	try {
		new Uint8Array(buffer);
	} catch {
		fail8('InputSchema', 'Protocol8 detached buffer');
	}
}
