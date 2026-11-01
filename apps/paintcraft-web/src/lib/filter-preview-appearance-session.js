import {
	decimal7,
	equal7,
	fail7,
	freeze7,
	numericalFacts7,
	shape7,
	snapshot7,
	validateBegin7,
	verifyBegin7,
	verifyBinding7,
} from './filter-preview-appearance-protocol.js';
import { visualCopy7 } from './filter-preview-appearance-visual-copy.js';
const cancelled = () =>
	fail7('OperationCancelled', 'Source7 generation retired');
function identity(request) {
	return {
		operation: request.operation,
		target: request.target,
		candidateRequirements: request.candidateRequirements,
		unionRequirements: request.unionRequirements,
	};
}
export function appearanceSession7(channel, plan, helloNumerical) {
	let latest = -1n,
		cut = -1n,
		wanted = null,
		running = null,
		barrier = null,
		held = false,
		closing = false,
		closePromise = null,
		fixed = null,
		sentCut = -1n,
		lastAck = null;
	function current(job) {
		if (closing || job.generation !== latest || job.generation <= cut)
			throw cancelled();
	}
	function retire(generation) {
		const value = decimal7(generation);
		if (value > cut) cut = value;
		if (wanted && wanted.generation <= cut) {
			wanted.reject(cancelled());
			wanted = null;
		}
		if (barrier) return barrier;
		if (value <= sentCut) return Promise.resolve(lastAck);
		// One barrier and one latest cut; never an unbounded promise chain.
		const task = (async () => {
			while (sentCut < cut) {
				const through = cut,
					text = through.toString();
				const reply = await channel.send('cancel', {
					generation: text,
				});
				shape7(reply, ['kind', 'generation']);
				if (reply.kind !== 'retired' || reply.generation !== text)
					throw fail7(
						'CodecFailed',
						'Source7 retirement acknowledgment',
					);
				sentCut = through;
				lastAck = reply;
			}
			return lastAck;
		})();
		barrier = task;
		task.then(
			() => {
				if (barrier === task) {
					barrier = null;
					pump();
				}
			},
			(error) => channel.abort(error),
		);
		return task;
	}
	function hold(result) {
		held = true;
		let released = false;
		result.release = () => {
			if (!released) {
				released = true;
				held = false;
				pump();
			}
		};
		return result;
	}
	async function execute(job) {
		const request = await verifyBegin7(job.request, plan);
		current(job);
		if (job.copy)
			return visualCopy7({
				channel,
				plan,
				request,
				copy: job.copy,
				check: () => current(job),
				hold,
			});
		const reply = await channel.send('update', { request });
		current(job);
		if (reply.kind === 'retired') {
			shape7(reply, ['kind', 'generation']);
			if (reply.generation !== request.generation)
				throw fail7('CodecFailed', 'Source7 retired update');
			throw cancelled();
		}
		const metadata = { ...reply };
		delete metadata.pixels;
		if (request.render) {
			if (
				!(reply.pixels instanceof Uint8Array) ||
				reply.pixels.byteOffset !== 0 ||
				!(reply.pixels.buffer instanceof ArrayBuffer) ||
				reply.pixels.byteLength !== reply.pixels.buffer.byteLength ||
				reply.pixels.length !== metadata.width * metadata.height * 4 ||
				reply.pixels.length > 16777216
			)
				throw fail7('CodecFailed', 'Source7 frame extent');
		} else if (Object.hasOwn(reply, 'pixels'))
			throw fail7('CodecFailed', 'Source7 prepared pixels');
		await verifyBinding7(metadata, request, plan);
		current(job);
		return request.render
			? hold({ ...metadata, pixels: reply.pixels })
			: { ...metadata, release() {} };
	}
	function pump() {
		if (closing || held || running || barrier || !wanted) return;
		const job = wanted;
		wanted = null;
		const task = execute(job);
		running = { job, task };
		task.then(job.resolve, (error) => {
			// A typed completed denial may be closed normally. Invalid metadata is
			// not trusted as joined cleanup: quarantine it.
			if (error.code === 'CodecFailed' || error.code === 'InputSchema')
				channel.abort(error);
			job.reject(error);
		})
			.finally(() => {
				if (running?.task === task) running = null;
				pump();
			})
			.catch(() => {});
	}
	function submit(input, copy) {
		try {
			if (closing || held || (copy && (running || wanted || barrier)))
				throw fail7('ResourceLimit', 'Source7 output slot busy');
			const request = freeze7(snapshot7(input, 8192));
			validateBegin7(request, plan);
			if (
				!equal7(
					numericalFacts7(request.unionRequirements),
					helloNumerical,
				)
			)
				throw fail7('CodecFailed', 'Source7 sealed HELLO union');
			const key = identity(request);
			if (fixed && !equal7(fixed, key))
				throw fail7(
					'ConflictStaleBase',
					'Source7 fixed intent requires close/reopen',
				);
			const generation = decimal7(request.generation);
			if (generation <= latest || generation <= cut)
				throw fail7(
					'ConflictStaleBase',
					'Source7 generation must increase',
				);
			fixed ??= key;
			latest = generation;
			if (wanted) wanted.reject(cancelled());
			const result = new Promise((resolve, reject) => {
				wanted = { request, generation, copy, resolve, reject };
			});
			if (running && !barrier)
				retire(running.job.request.generation).catch(() => {});
			pump();
			return result;
		} catch (error) {
			return Promise.reject(error);
		}
	}
	return Object.freeze({
		binding: plan,
		update: (request) => submit(request),
		base(request) {
			if (request?.operation !== null || request?.target !== null)
				return Promise.reject(
					fail7(
						'InputSchema',
						'Source7 BASE requires null operation and target',
					),
				);
			return submit(request);
		},
		encodedCopy(request, output, selection = new Uint8Array()) {
			try {
				const copy = {
					output: snapshot7(output, 4096),
					selection: null,
				};
				if (
					!(selection instanceof Uint8Array) ||
					!(selection.buffer instanceof ArrayBuffer) ||
					selection.byteOffset !== 0 ||
					selection.byteLength !== selection.buffer.byteLength ||
					selection.byteLength > 4194304 ||
					selection.buffer.resizable === true
				)
					throw fail7('InputSchema', 'Source7 exclusive copy mask');
				copy.selection = structuredClone(selection, {
					transfer: [selection.buffer],
				});
				return submit(request, copy);
			} catch (error) {
				return Promise.reject(error);
			}
		},
		cancel(generation) {
			try {
				return retire(generation);
			} catch (error) {
				return Promise.reject(error);
			}
		},
		close() {
			if (closePromise) return closePromise;
			if (held)
				return Promise.reject(
					fail7(
						'ResourceLimit',
						'Release Source7 output before close',
					),
				);
			closing = true;
			closePromise = (async () => {
				await retire(latest < 0n ? '0' : latest.toString());
				if (running) {
					try {
						await running.task;
					} catch {
						/* Joined cancellation or typed completed denial. */
					}
				}
				const ack = await channel.close();
				channel.releaseJoined();
				return ack;
			})().catch((error) => {
				channel.abort(error);
				throw error;
			});
			closePromise.catch(() => {});
			return closePromise;
		},
	});
}
