import { fail7, shape7, uint7 } from './filter-preview-appearance-protocol.js';
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
// One channel/Worker; termination without the genuine closed ACK quarantines it.
export function appearanceChannel7(worker, lease, timeoutMs = 120000) {
	let next = 0,
		failed = null,
		joined = false;
	const pending = new Map();
	function abort(error) {
		if (failed || joined) return;
		failed = error;
		lease.quarantine();
		worker.terminate();
		for (const slot of pending.values()) {
			clearTimeout(slot.timer);
			slot.reject(error);
		}
		pending.clear();
	}
	worker.onerror = () => abort(fail7('CodecFailed', 'Source7 Worker trap'));
	worker.onmessageerror = () =>
		abort(fail7('CodecFailed', 'Source7 Worker message transfer'));
	worker.onmessage = ({ data }) => {
		let slot;
		try {
			uint7(data?.id, 0xffffffff, 1);
			slot = pending.get(data.id);
			if (!slot)
				throw fail7('CodecFailed', 'Source7 unexpected correlation');
			pending.delete(data.id);
			clearTimeout(slot.timer);
			if (data.kind === 'error') {
				shape7(data, ['kind', 'id', 'completed', 'code', 'message']);
				if (
					data.completed !== true ||
					!CODES.has(data.code) ||
					typeof data.message !== 'string' ||
					data.message.length > 4096
				) {
					const error = fail7(
						'CodecFailed',
						'Source7 unjoined Worker error',
					);
					slot.reject(error);
					abort(error);
					return;
				}
				slot.reject(fail7(data.code, data.message));
				return;
			}
			const metadata = { ...data };
			delete metadata.id;
			slot.resolve(metadata);
		} catch (error) {
			slot?.reject(error);
			abort(error);
		}
	};
	return Object.freeze({
		abort,
		send(kind, body = {}, transfer = []) {
			if (failed || joined)
				return Promise.reject(
					failed ??
						fail7('OperationCancelled', 'Source7 channel closed'),
				);
			if (pending.size >= 3 || next === 0xffffffff)
				return Promise.reject(
					fail7('ResourceLimit', 'Source7 bounded control queue'),
				);
			const id = ++next;
			return new Promise((resolve, reject) => {
				const timer = setTimeout(
					() =>
						abort(
							fail7(
								'CodecFailed',
								'Source7 missing acknowledgment',
							),
						),
					timeoutMs,
				);
				pending.set(id, { resolve, reject, timer });
				try {
					worker.postMessage({ kind, id, ...body }, transfer);
				} catch (error) {
					abort(error);
				}
			});
		},
		releaseJoined() {
			if (!joined || failed)
				throw fail7('CodecFailed', 'Source7 release before join');
			lease.release();
		},
		async close() {
			const reply = await this.send('close');
			shape7(reply, ['kind']);
			if (reply.kind !== 'closed') {
				const error = fail7(
					'CodecFailed',
					'Source7 close acknowledgment',
				);
				abort(error);
				throw error;
			}
			joined = true;
			for (const slot of pending.values()) {
				clearTimeout(slot.timer);
				slot.reject(
					fail7('OperationCancelled', 'Source7 joined close'),
				);
			}
			pending.clear();
			worker.terminate();
			return reply;
		},
	});
}
