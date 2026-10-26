// Inspect the actual Wasm memory declaration before publishing a codec asset.
export const LINEAR_MEMORY_BYTES = 256 * 1024 * 1024;
export const MEMORY_PAGES_MAX = LINEAR_MEMORY_BYTES / 65536;
export function memoryLimits(bytes) {
	if (!(bytes instanceof Uint8Array) || bytes.length > 25 * 1024 * 1024)
		throw new Error('Codec Wasm file size allowance');
	const magic = [0, 97, 115, 109, 1, 0, 0, 0];
	if (magic.some((value, index) => bytes[index] !== value))
		throw new Error('Invalid codec Wasm header');
	const module = new WebAssembly.Module(bytes);
	if (
		WebAssembly.Module.imports(module).some(
			(entry) => entry.kind === 'memory',
		)
	)
		throw new Error('Codec Wasm cannot import additional memory');
	let position = 8;
	function leb(end) {
		let value = 0;
		for (let i = 0; i < 5; i++) {
			if (position >= end) throw new Error('Truncated Wasm integer');
			const byte = bytes[position++];
			if (i === 4 && byte > 15) throw new Error('Wasm integer overflow');
			value += (byte & 127) * 2 ** (i * 7);
			if (!(byte & 128)) return value;
		}
		throw new Error('Wasm integer overflow');
	}
	let memory;
	while (position < bytes.length) {
		const section = bytes[position++];
		const length = leb(bytes.length);
		const end = position + length;
		if (end > bytes.length) throw new Error('Truncated Wasm section');
		if (section === 5) {
			if (memory || leb(end) !== 1 || leb(end) !== 1)
				throw new Error('Expected one unshared bounded Wasm32 memory');
			memory = { minimum: leb(end), maximum: leb(end) };
			if (position !== end) throw new Error('Extra Wasm memory bytes');
		}
		position = end;
	}
	if (
		!memory ||
		memory.maximum !== MEMORY_PAGES_MAX ||
		memory.minimum > memory.maximum
	)
		throw new Error('Codec Wasm must declare exactly a 256 MiB maximum');
	return memory;
}
