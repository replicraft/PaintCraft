// Actual built Astro graph, CSP and lean Worker. No public image route is enabled.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { createHash } from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { memoryLimits } from './codec-memory.mjs';
import {
	platforms,
	assetName,
	releasesApi,
} from '../src/lib/download-platform.js';
const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
const dist = path.join(app, 'dist');
if (!process.env.PLAYWRIGHT_MODULE)
	throw new Error(
		'Set PLAYWRIGHT_MODULE to the existing Playwright ES module',
	);
const { chromium } = await import(
	pathToFileURL(path.resolve(process.env.PLAYWRIGHT_MODULE)).href
);
const evidence = {
	publicReaderEnabled: false,
	executionStopQualified: false,
	checks: [],
};
const output = process.env.CODEC_EVIDENCE_DIR;
let attemptDirectory;
if (output) {
	fs.mkdirSync(output, { recursive: true });
	attemptDirectory = fs.mkdtempSync(path.join(output, 'attempt-'));
	evidence.startedAt = new Date().toISOString();
	evidence.sourceSha256 = {};
	for (const name of [
		'src/lib/codec-worker.js',
		'src/lib/codec-worker-protocol.js',
		'src/lib/codec-bridge.js',
		'src/lib/main.js',
		'src/pages/index.astro',
		'src/paintcraft-web/lib.rs',
		'astro.config.mjs',
		'package.json',
		'scripts/codec-production-browser.mjs',
	]) {
		const bytes = fs.readFileSync(path.join(app, name));
		evidence.sourceSha256[name] = createHash('sha256')
			.update(bytes)
			.digest('hex');
		const target = path.join(attemptDirectory, 'sources', name);
		fs.mkdirSync(path.dirname(target), { recursive: true });
		fs.writeFileSync(target, bytes);
	}
}

let browser;
let server;
try {
	evidence.artifactSha256 = {};
	function inventory(directory) {
		for (const entry of fs.readdirSync(directory, {
			withFileTypes: true,
		})) {
			const file = path.join(directory, entry.name);
			if (entry.isDirectory()) inventory(file);
			else if (entry.isFile()) {
				const bytes = fs.readFileSync(file);
				evidence.artifactSha256[path.relative(dist, file)] = {
					bytes: bytes.length,
					sha256: createHash('sha256').update(bytes).digest('hex'),
				};
			}
		}
	}
	inventory(dist);
	const artifacts = fs.readdirSync(path.join(dist, '_astro'));
	const codecWasm = artifacts.find(
		(name) =>
			name.startsWith('paintcraft_web_codec_worker_bg') &&
			name.endsWith('.wasm'),
	);
	const codecRuntime = artifacts.find(
		(name) =>
			name.startsWith('codec-worker-runtime-') && name.endsWith('.js'),
	);
	if (!codecWasm || !codecRuntime)
		throw new Error('Production graph omitted lean Worker/Wasm');
	evidence.artifacts = {
		codecWasm,
		codecRuntime,
		...memoryLimits(fs.readFileSync(path.join(dist, '_astro', codecWasm))),
	};
	const fixture = (name) =>
		path.join(
			root,
			'crates/services/paintcraft-formats/tests/jxl-fixtures',
			name,
		);
	server = http.createServer((request, response) => {
		const pathname = new URL(request.url, 'http://localhost').pathname;
		let file;
		if (pathname.startsWith('/__qa/'))
			file = fixture(path.basename(pathname));
		else if (pathname === '/__download') {
			response.setHeader(
				'Content-Disposition',
				'attachment; filename="paintcraft-test.dmg"',
			);
			response.end('mock release asset');
			return;
		} else
			file = path.join(
				dist,
				pathname.endsWith('/') ? `${pathname}index.html` : pathname,
			);
		if (
			!file.startsWith(dist + path.sep) &&
			!file.startsWith(path.dirname(fixture('rgba8.jxl')) + path.sep)
		) {
			response.writeHead(403);
			response.end();
			return;
		}
		if (!fs.existsSync(file)) {
			response.writeHead(404);
			response.end();
			return;
		}
		const extension = path.extname(file);
		response.setHeader(
			'Content-Type',
			{
				'.html': 'text/html',
				'.js': 'text/javascript',
				'.wasm': 'application/wasm',
				'.json': 'application/json',
				'.css': 'text/css',
			}[extension] ?? 'application/octet-stream',
		);
		fs.createReadStream(file).pipe(response);
	});
	await new Promise((resolve, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', resolve);
	});
	const origin = `http://127.0.0.1:${server.address().port}`;
	browser = await chromium.launch({
		channel: 'chrome',
		headless: true,
		args: ['--enable-unsafe-swiftshader'],
	});
	const cdp = await browser.newBrowserCDPSession();
	evidence.browser = {
		version: browser.version(),
		cdp: await cdp.send('Browser.getVersion'),
	};
	async function page() {
		const page = await browser.newPage({ acceptDownloads: true });
		const errors = [];
		page.on('pageerror', (error) => errors.push(String(error)));
		await page.addInitScript(() => {
			window.__csp = [];
			document.addEventListener('securitypolicyviolation', (event) =>
				window.__csp.push({
					directive: event.effectiveDirective,
					blocked: event.blockedURI,
				}),
			);
		});
		await page.goto(origin);
		await page.waitForFunction(
			() =>
				window.paintcraft?.codec_service?.() &&
				document.querySelector('#loading')?.hidden,
			{ timeout: 60000 },
		);
		if (errors.length) throw new Error(errors.join('\n'));
		return page;
	}
	const positive = await page();
	evidence.positive = await positive.evaluate(
		async ({ runtime }) => {
			const app = window.paintcraft;
			const service = app.codec_service();
			const owner = {
				session: '00000000-0000-4000-8000-000000000001',
				state: '00000000-0000-4000-8000-000000000002',
				epoch: 17n,
				frontend: 3,
			};
			let mismatch = false;
			try {
				app.install_codec_service({
					applicationVersion: () => '999.0.0',
					protocolVersion: () => 2,
				});
			} catch {
				mismatch = true;
			}
			if (!mismatch || app.codec_service() !== service)
				throw new Error('Version mismatch replaced installed service');
			service.setOwner(owner);
			const bytes = await (await fetch('/__qa/rgba8.jxl')).arrayBuffer();
			const result = await service.decodeFile(
				new File([bytes], 'painting.jxl'),
				owner,
			);
			const pixels = Array.from(result.rgba);
			const expected = [
				10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120,
				128, 130, 140, 150, 254, 160, 170, 180, 255,
			];
			if (
				result.width !== 3 ||
				result.height !== 2 ||
				JSON.stringify(pixels) !== JSON.stringify(expected)
			)
				throw new Error('Actual production JXL oracle mismatch');
			if (!service.budget().reservedBytes)
				throw new Error('Prepared result lost admission');
			result.release();
			if (service.budget().reservedBytes)
				throw new Error('Consumed result retained logical charge');
			const bad = await (await fetch('/__qa/rgba16.jxl')).arrayBuffer();
			let refused = false;
			try {
				await service.decodeFile(
					new File([bad], 'high-depth.jxl'),
					owner,
				);
			} catch (error) {
				refused = error.code === 'CapabilityUnsupported';
			}
			if (
				!refused ||
				service.budget().reservedBytes ||
				service.budget().quarantined
			)
				throw new Error(
					'Canonical refusal did not release completed job',
				);
			const memory = await new Promise((resolve, reject) => {
				const worker = new Worker(runtime, { type: 'module' });
				const timer = setTimeout(() => {
					worker.terminate();
					reject(new Error('Memory probe timeout'));
				}, 10000);
				worker.onerror = () => {
					clearTimeout(timer);
					worker.terminate();
					reject(new Error('Memory probe failed'));
				};
				worker.onmessage = ({ data }) => {
					clearTimeout(timer);
					worker.terminate();
					resolve(data);
				};
				worker.postMessage({ kind: 'probe-memory' });
			});
			if (memory.kind !== 'memory' || !memory.growthRefused)
				throw new Error('Actual Wasm growth limit not enforced');
			if (window.__csp.length)
				throw new Error(
					`Unexpected CSP violations: ${JSON.stringify(window.__csp)}`,
				);
			const blob = URL.createObjectURL(
				new Blob(['self.postMessage("ran")'], {
					type: 'text/javascript',
				}),
			);
			const blobBlocked = await new Promise((resolve, reject) => {
				let worker;
				const timer = setTimeout(() => {
					worker?.terminate();
					reject(new Error('CSP blob test timeout'));
				}, 2000);
				try {
					worker = new Worker(blob);
					worker.onerror = (event) => {
						event.preventDefault();
						clearTimeout(timer);
						worker.terminate();
						resolve(true);
					};
					worker.onmessage = () => {
						clearTimeout(timer);
						worker.terminate();
						resolve(false);
					};
				} catch {
					clearTimeout(timer);
					resolve(true);
				}
			});
			URL.revokeObjectURL(blob);
			if (!blobBlocked)
				throw new Error('Production worker-src permitted blob Worker');
			return {
				version: app.application_version(),
				pixels,
				memory,
				blobBlocked,
				csp: document.querySelector(
					'meta[http-equiv="Content-Security-Policy"]',
				).content,
				fontAssets: performance
					.getEntriesByType('resource')
					.filter((entry) => entry.name.includes('/fonts/'))
					.map((entry) => entry.name),
			};
		},
		{ runtime: `${origin}/_astro/${codecRuntime}` },
	);
	evidence.checks.push(
		'hashed-production-worker-and-wasm',
		'exact-independent-rgba',
		'version-binding',
		'completed-refusal-release',
		'actual-memory-grow-refusal',
		'production-csp',
		'CJK-startup',
	);
	await positive.close();
	for (const mode of ['cancel', 'stale']) {
		const current = await page();
		evidence[mode] = await current.evaluate(async (mode) => {
			const service = window.paintcraft.codec_service();
			const owner = {
				session: '00000000-0000-4000-8000-000000000001',
				state: '00000000-0000-4000-8000-000000000002',
				epoch: 17n,
				frontend: 3,
			};
			service.setOwner(owner);
			const bytes = await (await fetch('/__qa/rgba8.jxl')).arrayBuffer();
			service.setOwner(owner);
			const file = new File([bytes], 'painting.jxl');
			const Original = window.Worker;
			let notify;
			const created = new Promise((resolve) => (notify = resolve));
			window.Worker = class extends Original {
				constructor(...args) {
					super(...args);
					notify();
				}
			};
			const pending = service.decodeFile(file, owner);
			await created;
			if (mode === 'cancel') service.cancel();
			else service.setOwner({ ...owner, epoch: 18n });
			let code;
			try {
				await pending;
				throw new Error('Unknown lifetime accepted result');
			} catch (error) {
				code = error.code;
			}
			window.Worker = Original;
			const budget = service.budget();
			if (!budget.quarantined || !budget.reservedBytes)
				throw new Error('Unknown lifetime was not quarantined');
			let next;
			try {
				await service.decodeFile(file, owner);
			} catch (error) {
				next = error.code;
			}
			if (next !== 'ResourceLimit')
				throw new Error('Quarantined realm accepted another job');
			return { code, budget, next, dirty: window.paintcraft.is_dirty() };
		}, mode);
		if (mode === 'cancel') {
			const download = current.waitForEvent('download', {
				timeout: 10000,
			});
			await current
				.locator('#paintcraft')
				.click({ position: { x: 300, y: 300 } });
			await current.keyboard.press(
				process.platform === 'darwin' ? 'Meta+s' : 'Control+s',
			);
			const saved = await download;
			evidence.quarantineDownload = saved.suggestedFilename();
			const savedBytes = fs.readFileSync(await saved.path());
			if (
				!savedBytes
					.subarray(0, 8)
					.equals(Buffer.from([80, 67, 78, 68, 1, 13, 10, 26]))
			)
				throw new Error('Quarantine download is not PCND');
			if (!evidence.quarantineDownload.endsWith('.pcnd'))
				throw new Error('Quarantine blocked PCND download');
		}
		await current.close();
	}
	evidence.checks.push(
		'actual-cancel-quarantine',
		'actual-stale-quarantine',
		'quarantine-refuses-new-job',
		'PCND-download-remains-available',
	);
	const download = await browser.newPage({ acceptDownloads: true });
	const release = {
		tag_name: 'v0.1.0',
		assets: platforms.flatMap((platform) =>
			platform.architectures.flatMap((architecture) =>
				platform.formats.map((format) => ({
					name: assetName(
						'0.1.0',
						`${platform.id}-${architecture.id}`,
						format.suffix,
					),
					browser_download_url: `${origin}/__download`,
					size: 1,
					state: 'uploaded',
				})),
			),
		),
	};
	await download.route(releasesApi, (route) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify(release),
		}),
	);
	await download.route(
		'https://github.com/replicraft/paintcraft/releases/download/**',
		(route) =>
			route.fulfill({
				status: 200,
				headers: {
					'content-type': 'application/octet-stream',
					'content-disposition':
						'attachment; filename="paintcraft-test.dmg"',
				},
				body: 'mock release asset',
			}),
	);
	const downloaded = download.waitForEvent('download', { timeout: 15000 });
	await download.goto(`${origin}/download/`);
	await downloaded;
	if (
		await download
			.locator('meta[http-equiv="Content-Security-Policy"]')
			.count()
	)
		throw new Error('App CSP leaked into download page');
	evidence.checks.push('download-page-release-fetch-and-autodownload');
	await download.close();
	if (attemptDirectory)
		fs.writeFileSync(
			path.join(attemptDirectory, 'production.json'),
			JSON.stringify(evidence, null, 4) + '\n',
		);
	console.log(
		JSON.stringify(
			{
				attemptDirectory,
				checks: evidence.checks,
				artifacts: evidence.artifacts,
				browser: evidence.browser,
				publicReaderEnabled: false,
				executionStopQualified: false,
			},
			null,
			4,
		),
	);
} catch (error) {
	evidence.failure = { message: String(error), stack: error.stack };
	if (attemptDirectory)
		fs.writeFileSync(
			path.join(attemptDirectory, 'failure.json'),
			JSON.stringify(evidence, null, 4) + '\n',
		);
	throw error;
} finally {
	await browser?.close();
	if (server) await new Promise((resolve) => server.close(resolve));
}
