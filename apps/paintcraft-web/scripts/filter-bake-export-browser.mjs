// Actual Studio bake consent and downloads on the production app; no Rust hooks.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
const dist = path.join(app, 'dist');
const kind = process.env.FILTER_BAKE_KIND ?? 'dds';
const phase = process.env.FILTER_BAKE_PHASE ?? 'formats';
assert.ok(['png', 'dds', 'msp'].includes(kind));
assert.ok(process.env.CODEC_EVIDENCE_DIR && process.env.PLAYWRIGHT_MODULE);
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'attempt-'),
);
fs.copyFileSync(
	fileURLToPath(import.meta.url),
	path.join(out, 'driver-source.mjs'),
);
const fixture = path.join(
	root,
	'crates/services/paintcraft-formats/tests/browser-filter-export-fixtures/invert.pcnd',
);
const oracle = JSON.parse(
	fs.readFileSync(path.join(path.dirname(fixture), 'oracle.json')),
).fixtures.find((row) => row.file === 'invert.pcnd');
const source = fs.readFileSync(fixture);
const sha = (b) => createHash('sha256').update(b).digest('hex');
assert.equal(sha(source), oracle.sha256);
const c = JSON.parse(process.env.FILTER_BAKE_COORDINATES ?? '{}');
const evidence = {
	status: 'started',
	kind,
	phase,
	coordinates: c,
	fixture: oracle,
	sourceInputs: {},
	assets: {},
};
function inventory(folder) {
	for (const row of fs.readdirSync(folder, { withFileTypes: true })) {
		const file = path.join(folder, row.name);
		if (row.isDirectory()) inventory(file);
		else
			evidence.assets[path.relative(dist, file)] = sha(
				fs.readFileSync(file),
			);
	}
}
inventory(dist);
for (const name of [
	'apps/paintcraft-web/scripts/filter-bake-export-browser.mjs',
	'crates/services/paintcraft-formats/tests/browser-filter-export-fixtures/generate.py',
	'Cargo.lock',
])
	evidence.sourceInputs[name] = sha(fs.readFileSync(path.join(root, name)));
let server, browser, page;
try {
	server = http.createServer((request, response) => {
		const url = new URL(request.url, 'http://localhost').pathname;
		const file = path.resolve(
			dist,
			`.${url.endsWith('/') ? `${url}index.html` : url}`,
		);
		if (!file.startsWith(`${dist}/`) || !fs.existsSync(file)) {
			response.writeHead(404);
			response.end();
			return;
		}
		response.setHeader(
			'Content-Type',
			{
				'.html': 'text/html',
				'.js': 'text/javascript',
				'.wasm': 'application/wasm',
				'.css': 'text/css',
				'.json': 'application/json',
			}[path.extname(file)] ?? 'application/octet-stream',
		);
		fs.createReadStream(file).pipe(response);
	});
	await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
	const { chromium } = await import(
		pathToFileURL(process.env.PLAYWRIGHT_MODULE).href
	);
	browser = await chromium.launch({
		channel: 'chrome',
		headless: true,
		args: ['--enable-unsafe-swiftshader'],
	});
	evidence.browser = browser.version();
	page = await browser.newPage({
		viewport: { width: 1360, height: 1000 },
		acceptDownloads: true,
	});
	await page.addInitScript(() => {
		localStorage.setItem('showWelcomeScreen', 'false');
		window.__bakeWorkers = [];
		const Original = window.Worker;
		window.Worker = class extends Original {
			constructor(...args) {
				super(...args);
				this.record = { url: String(args[0]), terminated: false };
				window.__bakeWorkers.push(this.record);
				this.addEventListener('message', (event) => {
					this.record.response = event.data?.kind;
					if (
						event.data?.bytes instanceof ArrayBuffer &&
						event.data.bytes.byteLength <= 65536
					)
						this.record.bytes = Array.from(
							new Uint8Array(event.data.bytes),
						);
				});
			}
			postMessage(value, transfer) {
				if (
					value.kind === 'dds-export' ||
					value.kind === 'msp-export'
				) {
					this.record.kind = value.kind;
					this.record.request = Array.from(
						new Uint8Array(value.bytes).slice(0, 224),
					);
				}
				super.postMessage(value, transfer);
			}
			terminate() {
				this.record.terminated = true;
				super.terminate();
			}
		};
	});
	await page.goto(`http://127.0.0.1:${server.address().port}/`, {
		waitUntil: 'networkidle',
	});
	await page.waitForFunction(
		() => window.paintcraft && document.querySelector('#loading').hidden,
		null,
		{ timeout: 60000 },
	);
	const click = async (x, y) => {
		await page.mouse.click(x, y);
		await page.waitForTimeout(450);
	};
	const point = async (key) => {
		assert.ok(c[key], `Supply observed ${key} coordinates`);
		await click(...c[key]);
	};
	const capture = async (name) =>
		page.screenshot({ path: path.join(out, `${name}.png`) });
	const download = async (name, trigger) => {
		const waiting = page.waitForEvent('download');
		await trigger();
		const result = await waiting;
		const file = path.join(out, name);
		await result.saveAs(file);
		await page.waitForTimeout(300);
		return fs.readFileSync(file);
	};
	await click(1273, 20);
	await click(1160, 98);
	const chooser = page.waitForEvent('filechooser');
	await click(20, 46);
	await click(80, 94);
	await (await chooser).setFiles(fixture);
	await page.waitForTimeout(700);
	const browserDownloads = [];
	page.on('download', (item) =>
		browserDownloads.push(item.suggestedFilename()),
	);
	const before = await download('before.pcnd', () =>
		page.keyboard.press('Meta+s'),
	);
	assert.equal(before[4], 3);
	evidence.before = sha(before);
	const title = await page.title();
	const dirty = await page.evaluate(() => window.paintcraft.is_dirty());
	const workers = () =>
		page.evaluate(() =>
			window.__bakeWorkers.filter(
				(w) => w.kind === 'dds-export' || w.kind === 'msp-export',
			),
		);
	const exportMenu = async () => {
		await click(20, 46);
		await click(80, 242);
	};
	await exportMenu();
	await capture('formats');
	if (phase !== 'formats') {
		if (kind !== 'png') await point('formatBox');
		if (c.formatScroll) {
			await page.mouse.move(c.formatBox[0], c.formatBox[1] + 90);
			await page.mouse.wheel(0, c.formatScroll);
			await page.waitForTimeout(450);
		}
		await capture('format-options');
		if (phase !== 'format-options') {
			if (kind !== 'png') await point('formatChoice');
			await capture('settings');
			if (phase !== 'settings') {
				if (kind === 'dds' || kind === 'msp') {
					await point('settingBox');
					await capture('setting-options');
					if (phase === 'setting-options') {
						evidence.status = 'calibration-only';
					} else await point('settingChoice');
				}
				await capture('generic-ready');
				if (phase !== 'setting-options' && phase !== 'generic-ready') {
					if (kind === 'dds' || kind === 'msp')
						await point('prepare');
					await capture('unchecked');
					assert.equal(
						(await workers()).length,
						0,
						'No actual Worker before explicit bake authority',
					);
					if (phase !== 'unchecked') {
						await point('bake');
						await page.waitForTimeout(800);
						await capture('checked');
						if (phase !== 'checked') {
							if (kind === 'dds' || kind === 'msp')
								await page.waitForFunction(
									() =>
										window.__bakeWorkers.some(
											(w) =>
												(w.kind === 'dds-export' ||
													w.kind === 'msp-export') &&
												w.response === 'result',
										),
									null,
									{ timeout: 15000 },
								);
							await page.waitForTimeout(700);
							await capture('ready');
							if (phase !== 'ready') {
								await point('readyBake');
								await page.waitForTimeout(600);
								await capture('revoked');
								await point(
									c.revokedDownload
										? 'revokedDownload'
										: 'download',
								);
								await page.waitForTimeout(600);
								assert.equal(
									browserDownloads.length,
									1,
									'Unchecked choice cannot download an old baked artifact',
								);
								const priorCount = (await workers()).length;
								await point(
									c.revokedBake ? 'revokedBake' : 'readyBake',
								);
								await page.waitForTimeout(800);
								if (kind === 'dds' || kind === 'msp')
									await page.waitForFunction(
										(count) =>
											window.__bakeWorkers.filter(
												(w) =>
													(w.kind === 'dds-export' ||
														w.kind ===
															'msp-export') &&
													w.response === 'result',
											).length > count,
										priorCount,
										{ timeout: 15000 },
									);
								await page.waitForTimeout(700);
								const copy = await download(
									`explicit.${kind}`,
									() => point('download'),
								);
								const after = await download('after.pcnd', () =>
									page.keyboard.press('Meta+s'),
								);
								assert.deepEqual(after, before);
								assert.equal(await page.title(), title);
								assert.equal(
									await page.evaluate(() =>
										window.paintcraft.is_dirty(),
									),
									dirty,
								);
								const trace = await workers();
								for (const w of trace) {
									assert.equal(w.request[9], 3);
									assert.equal(w.request[122], 1);
									assert.equal(w.bytes[9], 3);
									assert.equal(w.bytes[266], 1);
									assert.equal(w.bytes[10], 0);
									assert.ok(
										w.bytes[193] &
											(kind === 'dds' ? 64 : 8),
										'Actual receipt reports filters-baked',
									);
									assert.equal(
										w.bytes[193] &
											(kind === 'dds' ? 32 : 4),
										0,
										'No fake metadata omission on source without attachment',
									);
									assert.deepEqual(
										w.bytes.slice(288, 320),
										w.request.slice(128, 160),
									);
									assert.ok(w.terminated);
								}
								assert.ok(
									process.env.PILLOW_PYTHON,
									'Use existing independent Pillow interpreter',
								);
								const external = spawnSync(
									process.env.PILLOW_PYTHON,
									[
										'-c',
										'from PIL import Image; import PIL,json,sys; im=Image.open(sys.argv[1]);print(json.dumps(dict(version=PIL.__version__,size=im.size,format=im.format,rgba=im.convert("RGBA").tobytes().hex())))',
										path.join(out, `explicit.${kind}`),
									],
									{ encoding: 'utf8' },
								);
								assert.equal(
									external.status,
									0,
									external.stderr,
								);
								const decoded = JSON.parse(external.stdout);
								assert.equal(
									decoded.format,
									kind.toUpperCase(),
								);
								assert.deepEqual(decoded.size, [2, 1]);
								assert.deepEqual(
									Buffer.from(decoded.rgba, 'hex'),
									Buffer.from(oracle.expected_rgba),
									'Independent decode matches maintained white/black filtered oracle, not source black/white or double inversion',
								);
								evidence.output = {
									sha256: sha(copy),
									bytes: copy.length,
									decoder: decoded,
								};
								evidence.workers = trace;
								evidence.status = 'passed';
							}
						}
					}
				}
			}
		}
	}
	if (evidence.status === 'started') evidence.status = 'calibration-only';
	for (const [file, digest] of Object.entries(evidence.assets))
		assert.equal(sha(fs.readFileSync(path.join(dist, file))), digest);
	for (const [file, digest] of Object.entries(evidence.sourceInputs))
		assert.equal(sha(fs.readFileSync(path.join(root, file))), digest);
	assert.equal(sha(fs.readFileSync(fixture)), oracle.sha256);
} catch (error) {
	evidence.status = 'failed';
	evidence.error = String(error.stack ?? error);
	if (page)
		await page
			.screenshot({ path: path.join(out, 'failure.png') })
			.catch(() => {});
	process.exitCode = 1;
} finally {
	await browser?.close();
	await new Promise((resolve) =>
		server ? server.close(resolve) : resolve(),
	);
	fs.writeFileSync(
		path.join(out, 'production.json'),
		`${JSON.stringify(evidence, null, 2)}\n`,
	);
	console.log(out, evidence.status);
}
