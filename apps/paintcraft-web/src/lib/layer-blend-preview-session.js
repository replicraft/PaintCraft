// Final Main pixels require a real captured original-Main owner admission.
import { fail8, shape8 } from './layer-blend-preview-common.js';
import { geometry8, validateView8 } from './layer-blend-preview-protocol.js';
import { binding8 } from './layer-blend-preview-failure.js';
export function blendSession8(channel, open, mainOwner) {
	shape8(mainOwner, ['admitFrame', 'current', 'retire']);
	for (const fn of Object.values(mainOwner))
		if (typeof fn !== 'function')
			fail8('InputSchema', 'Source8 actual Main owner methods');
	let token = 0n,
		generation = 0n,
		active = null,
		held = null,
		closing = null;
	let lastDispatched = '0',
		retired = '0',
		activeJoin = Promise.resolve(),
		cancelBarrier = null;
	function current(request) {
		if (
			closing ||
			retired === request.token ||
			active !== request ||
			!mainOwner.current(request)
		)
			fail8('ConflictStaleBase', 'Source8 Main publication fence');
	}
	function retire(request) {
		retired = request.token;
		if (lastDispatched !== request.token) return activeJoin;
		if (cancelBarrier) return cancelBarrier;
		cancelBarrier = channel.cancel({
			kind: 'cancel',
			protocol: 8,
			id: 1,
			...binding8(open, request),
		});
		cancelBarrier
			.finally(() => {
				cancelBarrier = null;
			})
			.catch(() => {});
		return cancelBarrier;
	}
	return Object.freeze({
		async view(input) {
			if (active || held || closing)
				fail8('ConflictStaleBase', 'Source8 bounded Main view owner');
			if (
				token === 0xffffffffffffffffn ||
				generation === 0xffffffffffffffffn
			)
				fail8('ResourceLimit', 'Source8 operation identity exhausted');
			shape8(input, ['visible', 'level']);
			const g = geometry8(input.visible, input.level, open.canvas);
			const request = validateView8(
				{
					kind: 'view',
					protocol: 8,
					id: 1,
					...binding8(open),
					token: (++token).toString(),
					generation: (++generation).toString(),
					visible: input.visible,
					level: input.level,
				},
				open,
			);
			active = request;
			let finished;
			activeJoin = new Promise((resolve) => {
				finished = resolve;
			});
			let buffer = null,
				guard = null;
			try {
				current(request);
				// This callback must admit exact data/owner/phase overlap in the SAME
				// captured Rust Main factory BEFORE returning. Scalars grant nothing.
				guard = await mainOwner.admitFrame(
					g.size[0] * g.size[1] * 4,
					request,
				);
				if (!guard || typeof guard.release !== 'function')
					fail8('InputSchema', 'Source8 accounted Main frame guard');
				current(request);
				buffer = new ArrayBuffer(g.size[0] * g.size[1] * 4);
				let next = 0;
				const rendered = channel.view(request, (patch) => {
					if (closing || retired === request.token) return;
					if (!mainOwner.current(request)) {
						retire(request).catch(channel.abort);
						return;
					}
					current(request);
					if (patch.ordinal !== next)
						fail8(
							'ConflictStaleBase',
							'Source8 Main patch ordinal',
						);
					const [x0, y0, x1, y1] = patch.patch,
						width = x1 - x0;
					const source = new Uint8Array(patch.rgba),
						target = new Uint8Array(buffer);
					for (let row = 0; row < y1 - y0; row++)
						target.set(
							source.subarray(
								row * width * 4,
								(row + 1) * width * 4,
							),
							((y0 + row) * g.size[0] + x0) * 4,
						);
					next++;
					// ACK is after actual validated copy and current-owner fence.
					channel.ack({
						kind: 'ack',
						protocol: 8,
						id: 1,
						...binding8(open, request),
						ordinal: patch.ordinal,
						patch: patch.patch,
					});
				});
				lastDispatched = request.token;
				await rendered;
				current(request);
				if (next !== g.patches)
					fail8('CodecFailed', 'Source8 incomplete Main frame');
				const result = Object.freeze({
					width: g.size[0],
					height: g.size[1],
					region: g.region,
					level: request.level,
					get rgba() {
						if (!buffer)
							fail8(
								'OperationCancelled',
								'Source8 frame retired',
							);
						return buffer;
					},
					release() {
						if (!buffer) return;
						buffer = null;
						guard.release();
						guard = null;
						held = null;
					},
				});
				held = result;
				return result;
			} finally {
				active = null;
				try {
					if (!held) {
						buffer = null;
						guard?.release();
					}
				} finally {
					finished();
				}
			}
		},
		async cancel() {
			if (!active) return;
			await retire(active);
		},
		close() {
			if (closing) return closing;
			closing = (async () => {
				if (cancelBarrier) await cancelBarrier;
				await channel.close(lastDispatched);
				await activeJoin;
				held?.release();
				// Host joins actual source/output/account owners; no early broker refund.
				await mainOwner.retire();
			})();
			return closing;
		},
	});
}
