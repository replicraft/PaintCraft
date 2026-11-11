// Additional passive Source7 frame capture; install after observeWorkers.
export function observeMaskFrames() {
	const Base = window.Worker;
	window.Worker = class extends Base {
		constructor(...args) {
			super(...args);
			this.addEventListener('message', ({ data }) => {
				if (data?.protocol !== 7 || data.kind !== 'frame') return;
				const q = window.__mainBlendSmoke;
				try {
					const row = this.__smokeRecord.events.at(-1);
					if (row?.direction !== 'reply' || row.id !== data.id)
						throw Error('Source7 observation ordering');
					if (!ArrayBuffer.isView(data.pixels))
						throw Error('Source7 frame must own a pixel view');
					const p = data.pixels;
					if (
						p.byteLength > 1048576 ||
						q.bytes + p.byteLength > 4194304
					) {
						q.overflow = true;
						return;
					}
					q.bytes += p.byteLength;
					const bytes = new Uint8Array(
						p.buffer,
						p.byteOffset,
						p.byteLength,
					).slice();
					row.pixelsHex = Array.from(bytes, (b) =>
						b.toString(16).padStart(2, '0'),
					).join('');
					for (const key of ['width', 'height']) {
						if (!Number.isSafeInteger(data[key]))
							throw Error('Source7 geometry');
						row[key] = data[key];
					}
					if (
						!Array.isArray(data.region) ||
						data.region.length !== 4 ||
						!data.region.every(Number.isSafeInteger)
					)
						throw Error('Source7 region');
					row.region = data.region.slice();
				} catch (problem) {
					if (q.errors.length < 32) q.errors.push(String(problem));
					else q.overflow = true;
				}
			});
		}
	};
}
