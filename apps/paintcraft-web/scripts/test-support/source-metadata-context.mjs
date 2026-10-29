// Synthetic protocol facts only; not an editor policy or real PCND producer.
export function context(target, { present = false, omit = false } = {}) {
	const bytes = new Uint8Array(64);
	bytes[15] = 1;
	bytes[48] = Number(present) | (Number(omit) << 1);
	bytes[49] = target;
	bytes[50] = 1;
	if (present) {
		bytes.fill(7, 16, 48);
		const view = new DataView(bytes.buffer);
		view.setUint32(52, 192);
		view.setBigUint64(56, 1024n);
	}
	return bytes;
}
