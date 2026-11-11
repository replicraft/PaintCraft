// Bounded diagnostics over real production assets/Workers, not Main authority.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { inflateSync } from 'node:zlib';

export const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
export function inventory(directory) {
	const files = {};
	let total = 0;
	function visit(folder) {
		for (const entry of fs.readdirSync(folder, { withFileTypes: true })) {
			assert(
				!entry.isSymbolicLink(),
				'dist must not escape through symlinks',
			);
			const file = path.join(folder, entry.name);
			if (entry.isDirectory()) visit(file);
			else if (entry.isFile()) {
				const size = fs.statSync(file).size;
				assert(size <= 25 * 1024 * 1024);
				const bytes = fs.readFileSync(file);
				assert.equal(bytes.length, size);
				total += bytes.length;
				assert(total <= 512 * 1024 * 1024);
				files[path.relative(directory, file)] = {
					bytes: bytes.length,
					sha256: sha(bytes),
				};
				assert(Object.keys(files).length <= 4096);
			}
		}
	}
	visit(directory);
	assert(files['index.html'], 'build the production website first');
	return files;
}
export function serve(dist, files, requests, failures) {
	return http.createServer((request, response) => {
		try {
			assert(requests.length < 8192, 'bounded diagnostic HTTP requests');
			const url = new URL(request.url, 'http://localhost').pathname;
			const relative = url.endsWith('/')
				? `${url.slice(1)}index.html`
				: url.slice(1);
			const row = Object.hasOwn(files, relative) ? files[relative] : null;
			if (!row) {
				response.writeHead(404).end();
				return;
			}
			const file = path.resolve(dist, relative);
			assert(file.startsWith(dist + path.sep));
			const bytes = fs.readFileSync(file);
			assert.equal(bytes.length, row.bytes);
			assert.equal(sha(bytes), row.sha256);
			requests.push({ url, file: relative, ...row });
			const mime = {
				'.html': 'text/html',
				'.js': 'application/javascript',
				'.wasm': 'application/wasm',
				'.json': 'application/json',
				'.css': 'text/css',
				'.otf': 'font/otf',
				'.svg': 'image/svg+xml',
			};
			response.writeHead(200, {
				'Content-Type':
					mime[path.extname(file)] ?? 'application/octet-stream',
				'Cache-Control': 'no-store',
			});
			response.end(bytes);
		} catch (error) {
			if (failures.length < 32) failures.push(String(error));
			response.writeHead(500).end();
		}
	});
}
export function observeWorkers() {
	const q = {
		workers: [],
		events: 0,
		errors: [],
		bytes: 0,
		sequence: 0,
		overflow: false,
		faultOnce: false,
		faults: 0,
	};
	Object.defineProperty(window, '__mainBlendSmoke', { value: q });
	const Native = window.Worker;
	const hex = (bytes) =>
		Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
	function fail(error) {
		if (q.errors.length < 32) q.errors.push(String(error).slice(0, 2000));
		else q.overflow = true;
	}
	function copy(bytes) {
		if (
			bytes.byteLength > 1048576 ||
			q.bytes + bytes.byteLength > 4194304
		) {
			q.overflow = true;
			return null;
		}
		q.bytes += bytes.byteLength;
		return bytes.slice();
	}
	function meta(value) {
		const result = {};
		for (const key of [
			'kind',
			'protocol',
			'id',
			'token',
			'generation',
			'ordinal',
			'patchCount',
			'closed',
			'owner',
			'document',
			'sourceState',
			'draft',
			'sourceSha256',
			'profileSha256',
			'byteLength',
			'level',
		]) {
			const v = value?.[key];
			if (
				(typeof v === 'string' && v.length <= 256) ||
				(typeof v === 'number' && Number.isFinite(v)) ||
				typeof v === 'boolean'
			)
				result[key] = v;
		}
		for (const key of ['visible', 'patch']) {
			const v = value?.[key];
			if (
				Array.isArray(v) &&
				v.length === 4 &&
				v.every(Number.isSafeInteger)
			)
				result[key] = v.slice();
		}
		if (value?.detail)
			result.detail = {
				code: String(value.detail.code).slice(0, 128),
				message: String(value.detail.message).slice(0, 2000),
			};
		return result;
	}
	function append(worker, event) {
		if (++q.events > 4096) {
			q.overflow = true;
			return;
		}
		event.sequence = ++q.sequence;
		worker.events.push(event);
	}
	window.Worker = class extends Native {
		constructor(...args) {
			super(...args);
			const worker = {
				index: q.workers.length,
				url: String(args[0]),
				created: ++q.sequence,
				events: [],
				terminated: null,
			};
			if (q.workers.length < 32) q.workers.push(worker);
			else q.overflow = true;
			this.__smokeRecord = worker;
			this.addEventListener('message', ({ data }) => {
				try {
					const event = { direction: 'reply', ...meta(data) };
					if (data?.rgba instanceof ArrayBuffer) {
						const bytes = copy(new Uint8Array(data.rgba));
						if (bytes) event.rgbaHex = hex(bytes);
					}
					append(worker, event);
				} catch (error) {
					fail(error);
				}
			});
			this.addEventListener('error', (event) => {
				worker.error = String(event.message).slice(0, 2000);
			});
		}
		postMessage(...args) {
			const value = args[0],
				control = value?.control ?? value;
			let source, event;
			try {
				event = { direction: 'request', ...meta(control) };
				if (
					control?.protocol === 8 &&
					control.kind === 'open' &&
					value.source instanceof ArrayBuffer
				) {
					source = value.source;
					event.sourceBytes = source.byteLength;
					const bytes = copy(new Uint8Array(source));
					if (bytes)
						crypto.subtle.digest('SHA-256', bytes).then((hash) => {
							event.encodedSha256 = hex(new Uint8Array(hash));
						}, fail);
					if (q.faultOnce) {
						q.faultOnce = false;
						q.faults++;
						new Uint8Array(source)[0] ^= 1;
						event.injectedByte = 0;
					}
				}
				append(this.__smokeRecord, event);
			} catch (error) {
				fail(error);
			}
			try {
				return super.postMessage(...args);
			} finally {
				if (source && event) event.detached = source.byteLength === 0;
			}
		}
		terminate() {
			this.__smokeRecord.terminated = ++q.sequence;
			return super.terminate();
		}
	};
}
export function healthy(trace) {
	assert.equal(trace.overflow, false);
	assert.deepEqual(trace.errors, []);
	assert(
		trace.workers.every((w) => !w.error),
		'no real Worker error',
	);
}
export function openEvent(worker) {
	return worker.events.find(
		(e) =>
			e.protocol === 8 && e.direction === 'request' && e.kind === 'open',
	);
}
export function completed(worker) {
	const view = worker.events
		.filter(
			(e) =>
				e.direction === 'request' &&
				e.protocol === 8 &&
				e.kind === 'view',
		)
		.at(-1);
	if (!view) return null;
	const finish = worker.events.find(
		(e) =>
			e.direction === 'reply' &&
			e.protocol === 8 &&
			e.kind === 'complete' &&
			e.id === view.id &&
			e.token === view.token,
	);
	return finish ? { view, finish } : null;
}
export function assertFrame(worker, specimen) {
	const { view, finish } = completed(worker) ?? {};
	assert(view && finish, 'latest actual Main view completed');
	assert.equal(view.level, 0);
	assert.deepEqual(view.visible, [0, 0, ...specimen.canvas]);
	const patches = worker.events.filter(
		(e) =>
			e.direction === 'reply' &&
			e.protocol === 8 &&
			e.kind === 'patch' &&
			e.id === view.id &&
			e.token === view.token,
	);
	assert.equal(patches.length, 1);
	assert.equal(finish.patchCount, 1);
	const patch = patches[0];
	assert.equal(patch.ordinal, 0);
	assert.deepEqual(patch.patch, [0, 0, ...specimen.canvas]);
	assert.deepEqual(Buffer.from(patch.rgbaHex, 'hex'), specimen.rgba);
	const ack = worker.events.find(
		(e) =>
			e.direction === 'request' &&
			e.kind === 'ack' &&
			e.id > view.id &&
			e.token === view.token &&
			e.generation === view.generation &&
			e.ordinal === 0,
	);
	assert(
		ack && ack.sequence > patch.sequence && finish.sequence > ack.sequence,
		'real copy-before-ACK then completion',
	);
	const open = openEvent(worker);
	assert(open && open.detached);
	assert.equal(open.sourceBytes, specimen.bytes.length);
	assert.equal(open.encodedSha256, specimen.sha256);
	assert.equal(open.sourceSha256, specimen.sha256);
	return {
		worker: worker.index,
		token: view.token,
		rgba_sha256: sha(specimen.rgba),
		source_sha256: open.sourceSha256,
	};
}
export function opaqueMarker(bytes, clip, rgb = [28, 116, 220]) {
	assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
	let p = 8,
		width,
		height,
		channels;
	const parts = [];
	while (p < bytes.length) {
		assert(p + 12 <= bytes.length);
		const n = bytes.readUInt32BE(p),
			tag = bytes.subarray(p + 4, p + 8).toString();
		assert(p + 12 + n <= bytes.length);
		const b = bytes.subarray(p + 8, p + 8 + n);
		if (tag === 'IHDR') {
			width = b.readUInt32BE(0);
			height = b.readUInt32BE(4);
			assert.equal(b[8], 8);
			assert([2, 6].includes(b[9]));
			assert.equal(b[12], 0);
			channels = b[9] === 2 ? 3 : 4;
		}
		if (tag === 'IDAT') parts.push(b);
		p += 12 + n;
		if (tag === 'IEND') break;
	}
	assert(width > 0 && height > 0 && width * height <= 4194304);
	const stride = width * channels,
		raw = inflateSync(Buffer.concat(parts), {
			maxOutputLength: (stride + 1) * height,
		});
	assert.equal(raw.length, (stride + 1) * height);
	const pixels = Buffer.alloc(stride * height);
	for (let y = 0; y < height; y++)
		for (let x = 0; x < stride; x++) {
			const f = raw[y * (stride + 1)];
			assert(f <= 4);
			const a = x >= channels ? pixels[y * stride + x - channels] : 0,
				b = y ? pixels[(y - 1) * stride + x] : 0,
				c =
					x >= channels && y
						? pixels[(y - 1) * stride + x - channels]
						: 0;
			const q = a + b - c,
				ds = [Math.abs(q - a), Math.abs(q - b), Math.abs(q - c)],
				paeth =
					ds[0] <= ds[1] && ds[0] <= ds[2]
						? a
						: ds[1] <= ds[2]
							? b
							: c;
			pixels[y * stride + x] =
				(raw[y * (stride + 1) + x + 1] +
					[0, a, b, Math.floor((a + b) / 2), paeth][f]) &
				255;
		}
	for (let y = clip[1]; y + 4 <= Math.min(height, clip[3]); y++)
		for (let x = clip[0]; x + 4 <= Math.min(width, clip[2]); x++) {
			let good = true;
			for (let dy = 0; dy < 4 && good; dy++)
				for (let dx = 0; dx < 4; dx++) {
					const at = ((y + dy) * width + x + dx) * channels;
					if (
						!rgb.every((v, c) => Math.abs(pixels[at + c] - v) <= 1)
					) {
						good = false;
						break;
					}
				}
			if (good) return { x, y, size: 4, rgb };
		}
	throw Error(
		'Real opaque B publication marker absent from GUI-excluding canvas clip',
	);
}
