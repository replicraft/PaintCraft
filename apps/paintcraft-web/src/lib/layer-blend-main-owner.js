// Native JS delegates for Root-minted opaque Rust owners. This is not authority
// validation: only the admitted Main consumer may supply the actual class handle.
function method(handle, name) {
	const value = handle?.[name];
	if (typeof value !== 'function')
		throw new TypeError(`Source8 opaque owner requires ${name}`);
	return value;
}
export function mainBlendOwner8Facade(handle) {
	const admit = method(handle, 'admitFrame');
	const current = method(handle, 'current');
	const retire = method(handle, 'retire');
	let owner = handle;
	handle = null; // Do not leave a second parameter binding rooted in the facade.
	function live() {
		if (!owner)
			throw new TypeError('Source8 opaque owner already consumed');
		return owner;
	}
	return Object.freeze({
		admitFrame(bytes, request) {
			return Reflect.apply(admit, live(), [bytes, request]);
		},
		current(request) {
			return Reflect.apply(current, live(), [request]);
		},
		retire() {
			const consumed = live();
			owner = null; // Drop the facade root before entering the consuming Rust ABI.
			return Reflect.apply(retire, consumed, []);
		},
	});
}
export function mainBlendRetirementTick8() {
	// Poll funding/identity/work belongs to the SAME original Rust VIEW operation.
	return new Promise((resolve) => setTimeout(resolve, 25));
}
export function deferMainBlendCredit8(handle) {
	const release = method(handle, 'release');
	// No Rust Closure and no synchronous fallback. The bound credit remains held
	// if scheduling throws or this original release rejects/throws. The caller
	// must preserve quarantine and observe failure, never mint a replacement.
	return Promise.resolve().then(() => Reflect.apply(release, handle, []));
}
