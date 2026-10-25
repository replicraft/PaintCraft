// Font bytes stay on this device and are loaded only for the selected family.
let faces = [];
export function localFontsSupported() {
	return typeof window.queryLocalFonts === 'function';
}
export async function listLocalFonts(request) {
	if (!localFontsSupported()) return [];
	if (!request) {
		const permission = await navigator.permissions.query({
			name: 'local-fonts',
		});
		if (permission.state !== 'granted') return [];
	}
	faces = await window.queryLocalFonts();
	return [...new Set(faces.map((face) => face.family))].sort();
}
export async function loadLocalFamily(family) {
	const selected = faces.filter((face) => face.family === family);
	const result = [];
	let total = 0;
	for (const face of selected) {
		const blob = await face.blob();
		total += blob.size;
		if (blob.size > 32 * 1024 * 1024 || total > 64 * 1024 * 1024)
			throw new Error('Font family exceeds the 64 MiB limit');
		result.push(new Uint8Array(await blob.arrayBuffer()));
	}
	if (!result.length) throw new Error('Font family is no longer available');
	return result;
}
