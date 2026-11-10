// One Worker/channel; no timer, termination or live-count guess returns credit.
import { shape8, uint8, fail8 } from './layer-blend-preview-common.js';
import {
	validateReady8,
	validateOpened8,
	validateCancelled8,
	validateClosed8,
	validateComplete8,
	validatePatch8,
} from './layer-blend-preview-protocol.js';
import {
	validateFailure8,
	sourceIdentity8,
} from './layer-blend-preview-failure.js';
export function blendChannel8(worker, lease, timeoutMs = 120000) {
	let next = 0,
		failed = null,
		joined = false,
		closing = false,
		readySeen = false;
	let open = null,
		view = null,
		active = null,
		control = null,
		closeRequest = null;
	let readyResolve,
		readyReject,
		timer,
		closeTask = null;
	const ready = new Promise((a, b) => {
		readyResolve = a;
		readyReject = b;
	});
	ready.catch(() => {});
	function abort(error) {
		if (failed || joined) return;
		failed = error;
		clearTimeout(timer);
		lease.quarantine();
		worker.terminate();
		readyReject(error);
		active?.reject(error);
		control?.reject(error);
		active = null;
		control = null;
	}
	function arm() {
		clearTimeout(timer);
		timer = setTimeout(
			() =>
				abort(
					Object.assign(
						new Error('Source8 missing joined response'),
						{
							code: 'CodecFailed',
						},
					),
				),
			timeoutMs,
		);
	}
	arm();
	function id() {
		if (failed || joined || closing || next === 0xffffffff)
			fail8('OperationCancelled', 'Source8 channel unavailable');
		return ++next;
	}
	function post(m, source = null) {
		try {
			worker.postMessage({ control: m, source }, source ? [source] : []);
			arm();
		} catch (error) {
			abort(error);
			throw error;
		}
	}
	worker.onerror = () => abort(new Error('Source8 Worker trap'));
	worker.onmessageerror = () =>
		abort(new Error('Source8 Worker message transfer'));
	worker.onmessage = ({ data }) => {
		try {
			if (failed || joined) return;
			if (data?.kind === 'ready') {
				if (next !== 0 || readySeen)
					fail8('CodecFailed', 'Source8 late READY');
				const captured = validateReady8(data, data.runtimeFacts);
				readySeen = true;
				clearTimeout(timer);
				readyResolve(captured);
				return;
			}
			if (data?.kind === 'unjoined')
				fail8('CodecFailed', 'Source8 unjoined Rust/runtime failure');
			uint8(data?.id, 0xffffffff, 1);
			if (data.kind === 'error') {
				const request = control?.request ?? active?.request;
				if (!request)
					fail8('CodecFailed', 'Source8 unsolicited joined failure');
				validateFailure8(data, request, open, view);
				joined = true;
				clearTimeout(timer);
				const error = Object.assign(
					new Error(data.detail.message),
					data.detail,
					{ closed: true },
				);
				active?.reject(error);
				control?.reject(error);
				active = null;
				control = null;
				return;
			}
			if (data.kind === 'closed-empty') {
				shape8(data, ['kind', 'protocol', 'id']);
				if (
					!closeRequest ||
					closeRequest.kind !== 'close-empty' ||
					data.id !== closeRequest.id ||
					data.protocol !== 8
				)
					fail8('CodecFailed', 'Source8 empty close binding');
			} else if (data.kind === 'closed') {
				if (!closeRequest || !open)
					fail8('CodecFailed', 'Source8 unexpected CLOSED');
				validateClosed8(data, closeRequest, open);
			} else if (closing) {
				// The Worker posts earlier patch traffic before its joined CLOSED.
				// Retire it without copy/ACK/publication; original credit remains held.
				if (data.kind === 'patch')
					validatePatch8(data, view, open, active?.ordinal ?? 0);
				else
					fail8(
						'CodecFailed',
						'Source8 unexpected traffic during close',
					);
				return;
			} else if (data.kind === 'opened') {
				if (active?.request.kind !== 'open')
					fail8('CodecFailed', 'Source8 unexpected OPENED');
				validateOpened8(data, open);
				clearTimeout(timer);
				active.resolve(data);
				active = null;
				return;
			} else if (data.kind === 'patch') {
				if (active?.request.kind !== 'view')
					fail8('CodecFailed', 'Source8 unexpected patch');
				validatePatch8(data, view, open, active.ordinal);
				if (control?.request.kind === 'cancel') {
					arm();
					return;
				}
				control = null;
				active.onPatch(data);
				arm();
				return;
			} else if (data.kind === 'complete') {
				if (active?.request.kind !== 'view')
					fail8('CodecFailed', 'Source8 unexpected COMPLETE');
				validateComplete8(data, view, open);
				active.resolve(data);
				active = null;
				// An earlier COMPLETE can race a dispatched CANCEL. Keep its join
				// waiter and deadline until the actual terminal response arrives.
				if (control?.request.kind === 'cancel') arm();
				else {
					clearTimeout(timer);
					control = null;
				}
				return;
			} else if (data.kind === 'cancelled') {
				if (control?.request.kind !== 'cancel')
					fail8('CodecFailed', 'Source8 unexpected CANCELLED');
				validateCancelled8(data, control.request, view, open);
				clearTimeout(timer);
				active?.reject(
					Object.assign(new Error('Source8 view cancelled'), {
						code: 'OperationCancelled',
					}),
				);
				active = null;
				control.resolve(data);
				control = null;
				return;
			} else fail8('CodecFailed', 'Source8 unknown reply');
			joined = true;
			clearTimeout(timer);
			active?.reject(
				Object.assign(new Error('Source8 closed'), {
					code: 'OperationCancelled',
				}),
			);
			active = null;
			control.resolve(data);
			control = null;
		} catch (error) {
			abort(error);
		}
	};
	return Object.freeze({
		ready,
		abort,
		open(m, source) {
			if (open || active) fail8('ConflictStaleBase', 'Source8 OPEN once');
			open = { ...m, id: id() };
			return new Promise((resolve, reject) => {
				active = { request: open, resolve, reject };
				post(open, source);
			});
		},
		view(m, onPatch) {
			if (!open || active || control)
				fail8('ConflictStaleBase', 'Source8 channel busy');
			view = { ...m, id: id() };
			return new Promise((resolve, reject) => {
				active = {
					request: view,
					resolve,
					reject,
					onPatch,
					ordinal: 0,
				};
				post(view);
			});
		},
		ack(m) {
			if (active?.request.kind !== 'view' || control)
				fail8('ConflictStaleBase', 'Source8 ACK pending');
			const request = { ...m, id: id() };
			control = { request, reject: active.reject };
			active.ordinal++;
			post(request);
		},
		cancel(m) {
			if (!active || control?.request.kind === 'cancel')
				fail8('ConflictStaleBase', 'Source8 no cancellable view');
			const request = { ...m, id: id() };
			const promise = new Promise((resolve, reject) => {
				control = { request, resolve, reject };
				post(request);
			});
			if (control) control.promise = promise;
			return promise;
		},
		close(token = '0') {
			if (closeTask) return closeTask;
			closeTask = (async () => {
				// READY owns request zero; do not make genuine READY look late.
				await ready;
				if (control?.request.kind === 'cancel') await control.promise;
				if (joined) return { joined: true };
				const request = open
					? {
							kind: 'close',
							protocol: 8,
							id: id(),
							...sourceIdentity8(open),
							token,
						}
					: { kind: 'close-empty', protocol: 8, id: id() };
				closeRequest = request;
				closing = true;
				return new Promise((resolve, reject) => {
					control = { request, resolve, reject };
					post(request);
				});
			})();
			return closeTask;
		},
		releaseJoined() {
			if (!joined || failed)
				fail8('CodecFailed', 'Source8 no genuine joined close');
			lease.release();
			worker.terminate();
		},
	});
}
