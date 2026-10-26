// Explicit headless qualification. Requires a built lean Wasm package and an
// existing Playwright installation; never starts the editor or touches dist.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { memoryLimits } from './codec-memory.mjs';
const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
const args = process.argv.slice(2);
const verifyExecutionStop = args.includes('--verify-execution-stop');
if (verifyExecutionStop)
	args.splice(args.indexOf('--verify-execution-stop'), 1);
if (args.length && (args.length !== 2 || args[0] !== '--pkg'))
	throw new Error(
		'Usage: codec-worker-browser.mjs [--pkg directory] [--verify-execution-stop]',
	);
const pkg = path.resolve(args[1] ?? path.join(app, 'codec-worker-pkg'));
const playwright = process.env.PLAYWRIGHT_MODULE;
if (!playwright)
	throw new Error(
		'Set PLAYWRIGHT_MODULE to an existing Playwright ES module',
	);
const { chromium } = await import(pathToFileURL(path.resolve(playwright)).href);
const expected = JSON.parse(
	fs.readFileSync(path.join(pkg, 'codec-worker-build.json')),
).version;
const memory = memoryLimits(
	fs.readFileSync(path.join(pkg, 'paintcraft_web_codec_worker_bg.wasm')),
);
const directory = fs.mkdtempSync(
	path.join(os.tmpdir(), 'paintcraft-codec-browser-'),
);
let browser;
let server;
try {
	fs.mkdirSync(path.join(directory, 'src/lib'), { recursive: true });
	for (const name of [
		'codec-worker.js',
		'codec-worker-runtime.js',
		'codec-worker-protocol.js',
	])
		fs.copyFileSync(
			path.join(app, 'src/lib', name),
			path.join(directory, 'src/lib', name),
		);
	fs.copyFileSync(
		path.join(app, 'package.json'),
		path.join(directory, 'package.json'),
	);
	fs.cpSync(pkg, path.join(directory, 'codec-worker-pkg'), {
		recursive: true,
	});
	fs.mkdirSync(path.join(directory, 'fixtures'));
	for (const name of ['primary', 'grid', 'alpha'])
		fs.copyFileSync(
			path.join(
				root,
				`crates/services/paintcraft-formats/tests/heif-fixtures/${name}.heic`,
			),
			path.join(directory, `fixtures/${name}.heic`),
		);
	for (const name of [
		'rgba8',
		'single',
		'rgba16',
		'icc',
		'linear',
		'animation',
	])
		fs.copyFileSync(
			path.join(
				root,
				`crates/services/paintcraft-formats/tests/jxl-fixtures/${name}.jxl`,
			),
			path.join(directory, `fixtures/${name}.jxl`),
		);
	fs.writeFileSync(
		path.join(directory, 'busy.js'),
		`self.onmessage=({data})=>{const probe=new Int32Array(data.probe);Atomics.store(probe,0,1);while(true){Atomics.add(probe,1,1);}};`,
	);
	server = http.createServer((request, response) => {
		response.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
		response.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
		const name = decodeURIComponent(
			new URL(request.url, 'http://localhost').pathname,
		);
		if (name === '/') {
			response.setHeader('Content-Type', 'text/html');
			response.end(
				'<!doctype html><title>Isolated codec qualification</title>',
			);
			return;
		}
		const file = path.resolve(directory, `.${name}`);
		if (
			!file.startsWith(directory + path.sep) ||
			!fs.existsSync(file) ||
			!fs.statSync(file).isFile()
		) {
			response.writeHead(404);
			response.end();
			return;
		}
		response.setHeader(
			'Content-Type',
			file.endsWith('.wasm')
				? 'application/wasm'
				: file.endsWith('.json')
					? 'application/json'
					: file.endsWith('.js')
						? 'text/javascript'
						: 'application/octet-stream',
		);
		fs.createReadStream(file).pipe(response);
	});
	await new Promise((resolve, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', resolve);
	});
	browser = await chromium.launch({ channel: 'chrome', headless: true });
	const cdp = await browser.newBrowserCDPSession();
	const browserMetadata = {
		version: browser.version(),
		cdp: await cdp.send('Browser.getVersion'),
	};
	console.log(
		JSON.stringify(
			{ kind: 'browser-metadata', ...browserMetadata },
			null,
			4,
		),
	);
	const page = await browser.newPage();
	await page.goto(`http://127.0.0.1:${server.address().port}/`);
	const result = await page.evaluate(
		async ({ version, verifyExecutionStop }) => {
			const { decodeInWorker, codecWorkerBudget } = await import(
				'/src/lib/codec-worker.js'
			);
			const owner = {
				session: '00000000-0000-4000-8000-000000000001',
				state: '00000000-0000-4000-8000-000000000002',
				epoch: 17n,
				frontend: 3,
			};
			const source = async (name) =>
				new Uint8Array(
					await (await fetch(`/fixtures/${name}`)).arrayBuffer(),
				);
			function assert(ok, message) {
				if (!ok) throw new Error(message);
			}
			let checks = 0;
			async function code(task, expected) {
				try {
					const result = await task;
					result?.release?.();
					throw new Error(`Missing ${expected}`);
				} catch (error) {
					assert(
						error.code === expected,
						`${expected}: ${error.stack}`,
					);
					checks++;
				}
			}
			for (const [name, width, alpha] of [
				['primary', 32, false],
				['grid', 64, false],
				['alpha', 32, true],
			]) {
				const bytes = await source(`${name}.heic`);
				const frame = await decodeInWorker({
					applicationVersion: version,
					source: bytes,
					owner,
					codec: 1,
					pixels: 4194304,
					currentOwner: () => owner,
				});
				assert(
					frame.model === 'native-gray8' &&
						frame.width === width &&
						frame.height === 24,
					'HEIF native geometry',
				);
				for (let y = 0; y < 24; y++)
					for (let x = 0; x < width; x++)
						assert(
							frame.gray[y * width + x] ===
								(x < 32 ? (x * 7 + y * 11) % 256 : 17),
							'HEIF independent gray oracle',
						);
				assert(
					Boolean(frame.alpha) === alpha &&
						(!alpha || frame.alpha.every((v) => v === 17)),
					'HEIF native alpha',
				);
				assert(
					frame.encoding.primaries === 2 &&
						frame.encoding.transfer === 2 &&
						frame.encoding.matrix === 2,
					'HEIF encoding retained',
				);
				assert(
					codecWorkerBudget().reservedBytes > 256 * 1024 * 1024,
					'Result admission retained',
				);
				frame.release();
				checks++;
			}
			const rgbaSource = await source('rgba8.jxl');
			const rgba = await decodeInWorker({
				applicationVersion: version,
				source: rgbaSource,
				owner,
				codec: 2,
				pixels: 6,
				currentOwner: () => owner,
			});
			assert(
				[...rgba.rgba].join(',') ===
					[
						10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110,
						120, 128, 130, 140, 150, 254, 160, 170, 180, 255,
					].join(','),
				'JXL independent RGBA oracle',
			);
			await code(
				decodeInWorker({
					applicationVersion: version,
					source: rgbaSource,
					owner,
					codec: 2,
					pixels: 6,
					currentOwner: () => owner,
				}),
				'ResourceLimit',
			);
			rgba.release();
			checks++;
			const single = await decodeInWorker({
				applicationVersion: version,
				source: await source('single.jxl'),
				owner,
				codec: 2,
				pixels: 1,
				currentOwner: () => owner,
			});
			assert(
				[...single.rgba].join(',') === '17,29,91,137',
				'JXL single pixel oracle',
			);
			single.release();
			checks++;
			for (const name of ['rgba16', 'icc', 'linear', 'animation'])
				await code(
					decodeInWorker({
						applicationVersion: version,
						source: await source(`${name}.jxl`),
						owner,
						codec: 2,
						pixels: 4194304,
						currentOwner: () => owner,
					}),
					'CapabilityUnsupported',
				);
			const probe = await new Promise((resolve, reject) => {
				const worker = new Worker('/src/lib/codec-worker-runtime.js', {
					type: 'module',
				});
				const timer = setTimeout(() => {
					worker.terminate();
					reject(new Error('Memory probe timed out'));
				}, 10000);
				worker.onmessage = ({ data }) => {
					clearTimeout(timer);
					worker.terminate();
					resolve(data);
				};
				worker.onerror = () => {
					clearTimeout(timer);
					worker.terminate();
					reject(new Error('Memory probe failed'));
				};
				worker.postMessage({ kind: 'probe-memory' });
			});
			assert(
				probe.kind === 'memory' &&
					probe.growthRefused &&
					probe.initialBytes <= 256 * 1024 * 1024 &&
					probe.version === version,
				'Actual Wasm memory.grow cap',
			);
			checks++;
			return {
				checks,
				memoryProbe: probe,
				executionStopQualified: false,
				publicReaderEnabled: false,
			};
		},

		{ version: expected, verifyExecutionStop },
	);
	const lifecycle = [];
	for (const mode of ['deadline', 'cancel', 'stale', 'tamper']) {
		const isolated = await browser.newPage();
		try {
			await isolated.goto(`http://127.0.0.1:${server.address().port}/`);
			lifecycle.push(
				await isolated.evaluate(
					async ({ version, mode, verifyExecutionStop }) => {
						const { decodeInWorker, codecWorkerBudget } =
							await import('/src/lib/codec-worker.js');
						const owner = {
							session: '00000000-0000-4000-8000-000000000001',
							state: '00000000-0000-4000-8000-000000000002',
							epoch: 17n,
							frontend: 3,
						};
						let current = owner,
							terminated = 0;
						const source = new Uint8Array(
							await (
								await fetch('/fixtures/rgba8.jxl')
							).arrayBuffer(),
						);
						const probe = new Int32Array(new SharedArrayBuffer(8));
						const abort = new AbortController();
						function assert(ok, message) {
							if (!ok) throw new Error(message);
						}
						const options = {
							applicationVersion: version,
							source,
							owner,
							codec: 2,
							pixels: 6,
							currentOwner: () => current,
						};
						let timer;
						const createWorker = () => {
							const worker = new Worker(
								mode === 'tamper'
									? '/src/lib/codec-worker-runtime.js'
									: '/busy.js',
								{ type: 'module' },
							);
							const proxy = {
								terminate() {
									terminated++;
									worker.terminate();
								},
								postMessage(data, transfer) {
									worker.postMessage(
										mode === 'tamper'
											? data
											: { ...data, probe: probe.buffer },
										transfer,
									);
								},
							};
							worker.onmessage = ({ data }) => {
								if (mode === 'tamper' && data.bytes)
									new Uint8Array(data.bytes)[88] ^= 1;
								proxy.onmessage?.({ data });
							};
							worker.onerror = (event) => proxy.onerror?.(event);
							if (mode === 'cancel' || mode === 'stale')
								timer = setInterval(() => {
									if (Atomics.load(probe, 0)) {
										clearInterval(timer);
										if (mode === 'cancel') abort.abort();
										else current = { ...owner, epoch: 18n };
									}
								}, 2);
							return proxy;
						};
						const expected =
							mode === 'deadline'
								? 'ResourceLimit'
								: mode === 'cancel'
									? 'OperationCancelled'
									: 'ConflictStaleBase';
						let code;
						try {
							await decodeInWorker({
								...options,
								deadlineMs: mode === 'deadline' ? 250 : 10000,
								signal: abort.signal,
								createWorker,
							});
						} catch (error) {
							code = error.code;
						} finally {
							clearInterval(timer);
						}
						assert(
							code === expected && terminated === 1,
							`Wrong ${mode} refusal/termination request`,
						);
						const budget = codecWorkerBudget();
						assert(
							budget.quarantined &&
								budget.reservedBytes > 256 * 1024 * 1024,
							'Uncertain Worker lifetime must retain logical charge',
						);
						try {
							await decodeInWorker(options);
							throw new Error(
								'Quarantined realm admitted another job',
							);
						} catch (error) {
							assert(
								error.code === 'ResourceLimit',
								'Quarantined realm must refuse new work',
							);
						}
						if (mode !== 'tamper')
							assert(
								Atomics.load(probe, 0) === 1,
								'Actual Worker did not begin execution',
							);
						if (mode === 'deadline' && verifyExecutionStop) {
							const immediate = Atomics.load(probe, 1);
							await new Promise((resolve) =>
								setTimeout(resolve, 50),
							);
							assert(
								Atomics.load(probe, 1) === immediate,
								'Immediate Worker-stop assertion failed (preserved13959 assumption)',
							);
							const start = performance.now();
							for (;;) {
								const before = Atomics.load(probe, 1);
								await new Promise((r) => setTimeout(r, 50));
								if (Atomics.load(probe, 1) === before) break;
								assert(
									performance.now() - start < 2000,
									'Busy Worker did not stop after termination request',
								);
							}
						}
						return {
							mode,
							code,
							terminationRequested: terminated,
							quarantined: budget.quarantined,
							reservedBytes: budget.reservedBytes,
							executionStopQualified: false,
						};
					},
					{ version: expected, mode, verifyExecutionStop },
				),
			);
		} finally {
			await isolated.close();
		}
	}

	console.log(
		JSON.stringify(
			{
				...result,
				browserMetadata,
				lifecycle,
				memoryDeclaration: memory,
				wasmBytes: fs.statSync(
					path.join(pkg, 'paintcraft_web_codec_worker_bg.wasm'),
				).size,
			},
			null,
			4,
		),
	);
} finally {
	await browser?.close();
	await new Promise((resolve) =>
		server ? server.close(resolve) : resolve(),
	);
	fs.rmSync(directory, { recursive: true, force: true });
}
