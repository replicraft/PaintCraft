// Real advanced DDS/MSP UI, metadata choice and production Worker. No Rust test hooks.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { projectAttachment } from './lib/metadata-export-oracles.mjs';

const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
const dist = path.join(app, 'dist');
const base = process.env.CODEC_EVIDENCE_DIR;
if (!base || !process.env.PLAYWRIGHT_MODULE)
	throw Error('Set evidence directory and existing Playwright module');
fs.mkdirSync(base, { recursive: true });
const out = fs.mkdtempSync(path.join(base, 'attempt-'));
const phase = process.env.ADVANCED_METADATA_PHASE ?? 'formats';
const kind = process.env.ADVANCED_METADATA_KIND ?? 'dds';
const faultKind = process.env.ADVANCED_METADATA_FAULT ?? 'digest';
assert.ok(
	['digest', 'extent', 'intent', 'target', 'version', 'admission'].includes(
		faultKind,
	),
);
assert.ok(['dds', 'msp'].includes(kind));
const evidence = {
	status: 'started',
	phase,
	faultKind: phase === 'fault' ? faultKind : null,
	sources: {},
	assets: {},
	checks: [],
	startedAt: new Date().toISOString(),
};
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
function inventory(folder) {
	for (const entry of fs.readdirSync(folder, { withFileTypes: true })) {
		const file = path.join(folder, entry.name);
		if (entry.isDirectory()) inventory(file);
		else
			evidence.assets[path.relative(dist, file)] = {
				bytes: fs.statSync(file).size,
				sha256: sha(fs.readFileSync(file)),
			};
	}
}
inventory(dist);
for (const name of [
	'Cargo.lock',
	'apps/paintcraft-web/src/paintcraft-web/app.rs',
	'apps/paintcraft-web/src/paintcraft-web/app/exporting.rs',
	'apps/paintcraft-web/src/paintcraft-web/app/exporting/frozen.rs',
	'apps/paintcraft-web/src/paintcraft-web/files.rs',
	'apps/paintcraft-web/src/paintcraft-web/files/flat_export.rs',
	'apps/paintcraft-web/src/paintcraft-web/text_ui.rs',
	'crates/services/paintcraft-formats/src/export.rs',
	'crates/services/paintcraft-formats/src/export/metadata_policy.rs',
	'crates/presentation/paintcraft-ui/src/source_metadata_export.rs',
	'apps/paintcraft-web/scripts/advanced-metadata-export-browser.mjs',
])
	evidence.sources[name] = sha(fs.readFileSync(path.join(root, name)));
const fixtures = path.join(
	root,
	'crates/services/paintcraft-formats/tests/png-retention-fixtures',
);
const pngSource = path.join(fixtures, 'rgb8.png');
const source =
	kind === 'dds'
		? pngSource
		: path.join(
				root,
				'docs/validation/metadata-omission/browser-input/bilevel.pcnd',
			);
const expected = fs.readFileSync(
	kind === 'dds'
		? `${source}.rgba`
		: path.join(path.dirname(source), 'bilevel.rgba'),
);
const original = fs.readFileSync(source);
const profile = fs.readFileSync(
	path.join(root, 'assets/profiles/sRGB2014.icc'),
);
const oracle = JSON.parse(
	fs.readFileSync(path.join(fixtures, 'oracle.json')),
).fixtures.find((row) => row.file === 'rgb8.png');
if (kind === 'dds') {
	assert.equal(sha(original), oracle.sha256);
	assert.equal(sha(expected), oracle.rgba_sha256);
}
assert.equal(sha(profile), oracle.profile_sha256);
evidence.input =
	kind === 'dds'
		? { path: path.relative(root, source), ...oracle }
		: {
				path: path.relative(root, source),
				width: 4,
				height: 4,
				sha256: sha(original),
				rgba_sha256: sha(expected),
				provenance: JSON.parse(
					fs.readFileSync(
						path.join(path.dirname(source), 'input.json'),
					),
				),
			};
for (const name of [
	'apps/paintcraft-web/scripts/lib/metadata-export-oracles.mjs',
	'crates/services/paintcraft-formats/tests/png-retention-fixtures/rgb8.png',
	'crates/services/paintcraft-formats/tests/png-retention-fixtures/rgb8.png.rgba',
	'crates/services/paintcraft-formats/tests/png-retention-fixtures/oracle.json',
	'assets/profiles/sRGB2014.icc',
])
	evidence.sources[name] = sha(fs.readFileSync(path.join(root, name)));
for (const file of [
	source,
	...(kind === 'msp'
		? [
				path.join(path.dirname(source), 'bilevel.rgba'),
				path.join(path.dirname(source), 'input.json'),
			]
		: []),
]) {
	evidence.sources[path.relative(root, file)] = sha(fs.readFileSync(file));
}
const { chromium } = await import(
	pathToFileURL(process.env.PLAYWRIGHT_MODULE).href
);
let browser, server, page;
try {
	server = http.createServer((request, response) => {
		const pathname = new URL(request.url, 'http://localhost').pathname;
		const file = path.resolve(
			dist,
			`.${pathname.endsWith('/') ? `${pathname}index.html` : pathname}`,
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
	browser = await chromium.launch({
		channel: 'chrome',
		headless: true,
		args: ['--enable-unsafe-swiftshader'],
	});
	evidence.browser = browser.version();
	const c = JSON.parse(process.env.ADVANCED_METADATA_COORDINATES ?? '{}');
	evidence.coordinates = c;
	async function freshPage() {
		page = await browser.newPage({
			viewport: { width: 1360, height: 1000 },
			acceptDownloads: true,
		});
		await page.addInitScript(
			(fault) => {
				localStorage.setItem('showWelcomeScreen', 'false');
				window.__metadataWorkers = [];
				const Original = window.Worker;
				window.Worker = class extends Original {
					constructor(...args) {
						super(...args);
						this.record = {
							url: String(args[0]),
							terminated: false,
						};
						window.__metadataWorkers.push(this.record);
						this.addEventListener('message', (event) => {
							this.record.response = event.data?.kind;
							if (
								event.data?.bytes instanceof ArrayBuffer &&
								event.data.bytes.byteLength <= 65536
							)
								this.record.responseBytes = Array.from(
									new Uint8Array(event.data.bytes),
								);
						});
					}
					postMessage(value, transfer) {
						if (
							value?.bytes instanceof ArrayBuffer &&
							value.bytes.byteLength <= 65536
						) {
							this.record.kind = value.kind;
							this.record.request = Array.from(
								new Uint8Array(value.bytes),
							);
						}
						if (
							fault.enabled &&
							(value.kind === 'dds-export' ||
								value.kind === 'msp-export')
						) {
							const bytes = new Uint8Array(value.bytes),
								view = new DataView(value.bytes);
							if (fault.kind === 'digest') bytes[160] ^= 1;
							else if (fault.kind === 'extent')
								view.setUint32(180, view.getUint32(180) - 1);
							else if (fault.kind === 'intent') bytes[176] = 1;
							else if (fault.kind === 'target')
								bytes[177] = bytes[177] === 1 ? 2 : 1;
							else if (fault.kind === 'version') {
								bytes[7] = 49;
								view.setUint16(8, 1);
							} else if (fault.kind === 'admission')
								view.setBigUint64(
									184,
									BigInt(view.getUint32(180)),
								);
							this.record.injectedFault = fault.kind;
							this.record.dispatchedHeader = Array.from(
								bytes.slice(0, 192),
							);
						}
						super.postMessage(value, transfer);
					}
					terminate() {
						this.record.terminated = true;
						return super.terminate();
					}
				};
			},
			{ enabled: phase === 'fault', kind: faultKind },
		);
		page.on('pageerror', (error) => {
			evidence.pageErrors ??= [];
			evidence.pageErrors.push(String(error));
		});
		await page.goto(`http://127.0.0.1:${server.address().port}/`, {
			waitUntil: 'networkidle',
		});
		await page.waitForFunction(
			() =>
				window.paintcraft && document.querySelector('#loading').hidden,
			null,
			{ timeout: 60000 },
		);
		await click(1273, 20);
		await click(1160, 98);
	}
	const click = async (x, y) => {
		await page.mouse.click(x, y);
		await page.waitForTimeout(400);
	};
	async function openFile(file) {
		const chooser = page.waitForEvent('filechooser');
		await click(20, 46);
		await click(80, 94);
		await (await chooser).setFiles(file);
		await page.waitForTimeout(700);
	}

	const clickPoint = async (name) => {
		assert.ok(c[name], `Set observed ${name} coordinate`);
		await click(...c[name]);
	};
	async function download(name, trigger) {
		const pending = page.waitForEvent('download');
		await trigger();
		const value = await pending;
		await value.saveAs(path.join(out, name));
		await page.waitForTimeout(400);
		return fs.readFileSync(path.join(out, name));
	}
	const workers = () =>
		page.evaluate(() =>
			window.__metadataWorkers.filter(
				(w) => w.kind === 'dds-export' || w.kind === 'msp-export',
			),
		);
	async function exportMenu() {
		await click(20, 46);
		await click(80, 242);
		await click(...(c.formatBox ?? [540, 414]));
		if (kind === 'msp' || c.formatScroll) {
			await page.mouse.move(540, 455);
			await page.mouse.wheel(0, -2000);
			await page.waitForTimeout(250);
			await page.mouse.wheel(0, c.formatScroll ?? 900);
			await page.waitForTimeout(200);
		}
	}
	async function selectAdvanced() {
		await exportMenu();
		await clickPoint('formatChoice');
		// Explicit codec settings persist from the first reviewed export.
		await clickPoint('prepare');
	}
	await freshPage();
	await openFile(source);
	if (kind === 'dds') {
		await click(415, 628);
		await click(448, 653);
	}
	await page.waitForTimeout(700);
	const before = await download('before.pcnd', () =>
		page.keyboard.press('Meta+s'),
	);
	const title = await page.title(),
		dirty = await page.evaluate(() => window.paintcraft.is_dirty());
	if (kind === 'dds')
		evidence.attachment = projectAttachment(
			before,
			original,
			profile,
			oracle.width,
			oracle.height,
		);
	else {
		const attachment = (bytes) => {
			let offset = 40;
			const number = () => {
				let n = 0,
					shift = 0;
				for (let i = 0; i < 8; i++) {
					const b = bytes[offset++];
					assert.notEqual(b, undefined);
					n += (b & 127) * 2 ** shift;
					if (b < 128) return n;
					shift += 7;
				}
				throw Error('PCND fixture varint');
			};
			assert.equal(number(), 4);
			assert.equal(number(), 4);
			const title = bytes[offset++];
			assert.ok(title === 0 || title === 1);
			if (title) {
				const length = number();
				offset += length;
			}
			const header = bytes.subarray(offset, offset + 192);
			assert.equal(header.subarray(0, 8).toString(), 'PCSMETA1');
			const length = Number(header.readBigUInt64BE(112));
			assert.equal(length, profile.length);
			return bytes.subarray(offset, offset + 192 + length);
		};
		assert.deepEqual(attachment(before), attachment(original));
		evidence.attachment = {
			sha256: sha(attachment(before)),
			currentDimensions: [4, 4],
			preservedOriginalDimensions: [3, 2],
		};
	}
	evidence.kind = kind;
	await exportMenu();
	await page.screenshot({ path: path.join(out, 'formats.png') });
	if (phase === 'formats') {
		evidence.status = 'calibration-only';
	} else {
		await clickPoint('formatChoice');
		await page.screenshot({ path: path.join(out, 'settings.png') });
		if (phase === 'settings') {
			evidence.status = 'calibration-only';
		} else {
			await clickPoint('settingBox');
			await page.screenshot({
				path: path.join(out, 'setting-options.png'),
			});
			if (phase === 'setting-options') {
				evidence.status = 'calibration-only';
			} else {
				await clickPoint('settingChoice');
				await page.screenshot({
					path: path.join(out, 'generic-ready.png'),
				});
				if (phase === 'generic-ready') {
					evidence.status = 'calibration-only';
				} else {
					await clickPoint('prepare');
					await page.screenshot({
						path: path.join(out, 'review-unchecked.png'),
					});
					assert.equal(
						(await workers()).length,
						0,
						'No Worker before explicit metadata choice',
					);
					if (phase === 'review') {
						evidence.status = 'calibration-only';
					} else {
						await clickPoint('omit');
						if (phase === 'fault') {
							await page.waitForFunction(
								() =>
									window.__metadataWorkers.some(
										(w) =>
											(w.kind === 'dds-export' ||
												w.kind === 'msp-export') &&
											Boolean(w.response),
									),
								null,
								{ timeout: 15000 },
							);
							await page.waitForTimeout(700);
							const trace = await workers();
							assert.equal(trace.length, 1);
							if (
								['intent', 'target', 'version'].includes(
									faultKind,
								)
							) {
								assert.equal(
									trace[0].response,
									'failure',
									'Malformed contract is not a typed completed response',
								);
								assert.equal(trace[0].responseBytes, undefined);
							} else {
								assert.equal(trace[0].response, 'result');
								const response = Buffer.from(
									trace[0].responseBytes,
								);
								assert.notEqual(
									response[10],
									0,
									'Typed refusal from real Worker',
								);
								if (faultKind === 'admission')
									assert.equal(
										response[10],
										3,
										'Under-admitted actual attachment yields ResourceLimit',
									);
								assert.equal(
									response.readUInt32BE(24),
									0,
									'No encoded image in refusal',
								);
								assert.equal(response.readUInt16BE(28), 0);
								assert.equal(response.readUInt16BE(30), 0);
							}
							assert.ok(trace[0].injectedFault);
							assert.equal(trace[0].terminated, true);
							await page.screenshot({
								path: path.join(out, 'tamper-refused.png'),
							});
							await page.keyboard.press('Escape');
							await page.waitForTimeout(400);
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
							evidence.workers = trace;
							evidence.checks.push(
								`Real production Worker refuses ${faultKind}, no output, terminates, unchanged source-bearing project/title/dirty`,
							);
							evidence.status = 'passed';
						} else {
							await page.waitForFunction(
								() =>
									window.__metadataWorkers.some(
										(w) =>
											(w.kind === 'dds-export' ||
												w.kind === 'msp-export') &&
											w.response === 'result',
									),
								null,
								{ timeout: 15000 },
							);
							await page.waitForTimeout(1000);
							await page.screenshot({
								path: path.join(out, 'ready.png'),
							});
							if (phase === 'ready') {
								evidence.status = 'calibration-only';
							} else {
								const first = await workers();
								assert.equal(first.length, 1);
								assert.equal(first[0].request[176], 3);
								assert.equal(
									first[0].request[177],
									kind === 'dds' ? 1 : 2,
								);
								assert.equal(first[0].request[8], 0);
								assert.equal(first[0].request[9], 2);
								await clickPoint('readyOmit');
								await page.waitForTimeout(600);
								assert.equal(
									(await workers()).length,
									1,
									'Unchecking revokes without starting another Worker',
								);
								await page.screenshot({
									path: path.join(out, 'revoked.png'),
								});
								await clickPoint('omit');
								await page.waitForFunction(
									() =>
										window.__metadataWorkers.filter(
											(w) =>
												(w.kind === 'dds-export' ||
													w.kind === 'msp-export') &&
												w.response === 'result',
										).length === 2,
									null,
									{ timeout: 15000 },
								);
								await page.waitForTimeout(700);
								const twice = await workers();
								assert.notDeepEqual(
									twice[0].request.slice(65, 81),
									twice[1].request.slice(65, 81),
									'Rechecked choice starts a new nonce, not resurrected old preview',
								);
								await clickPoint('cancel');
								const cancelled = await download(
									'cancel.pcnd',
									() => page.keyboard.press('Meta+s'),
								);
								assert.deepEqual(cancelled, before);
								await selectAdvanced();
								await clickPoint('omit');
								await page.waitForFunction(
									() =>
										window.__metadataWorkers.filter(
											(w) =>
												(w.kind === 'dds-export' ||
													w.kind === 'msp-export') &&
												w.response === 'result',
										).length === 3,
									null,
									{ timeout: 15000 },
								);
								await page.waitForTimeout(700);
								const copy = await download(
									`explicit.${kind}`,
									() => clickPoint('download'),
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
								assert.equal(copy.includes(profile), false);
								const trace = await workers();
								evidence.workers = trace;
								const mask = kind === 'dds' ? 32 : 4;
								for (const w of trace) {
									assert.equal(w.request[176], 3);
									assert.equal(w.responseBytes[9], 2);
									assert.ok(w.responseBytes[193] & mask);
									assert.deepEqual(
										w.responseBytes.slice(288, 352),
										w.request.slice(128, 192),
									);
									assert.equal(w.terminated, true);
								}
								if (kind === 'dds') {
									assert.equal(
										copy.subarray(0, 4).toString(),
										'DDS ',
									);
									const height = copy.readUInt32LE(12),
										width = copy.readUInt32LE(16);
									assert.deepEqual(
										[width, height],
										[oracle.width, oracle.height],
									);
									const offset =
										copy.subarray(84, 88).toString() ===
										'DX10'
											? 148
											: 128;
									assert.deepEqual(
										copy.subarray(
											offset,
											offset + expected.length,
										),
										expected,
										'Independent libpng raw top-level pixels preserved',
									);
								} else {
									assert.equal(
										copy.subarray(0, 4).toString(),
										'LinS',
									);
									assert.deepEqual(
										[
											copy.readUInt16LE(4),
											copy.readUInt16LE(6),
										],
										[4, 4],
									);
								}
								assert.ok(
									process.env.PILLOW_PYTHON,
									'Set existing independent Pillow oracle interpreter',
								);
								const external = spawnSync(
									process.env.PILLOW_PYTHON,
									[
										'-c',
										'from PIL import Image; import PIL,json,sys; image=Image.open(sys.argv[1]); print(json.dumps(dict(version=PIL.__version__,size=image.size,rgba=image.convert("RGBA").tobytes().hex())))',
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
								assert.deepEqual(
									Buffer.from(decoded.rgba, 'hex'),
									expected,
									'Independent Pillow decoded output matches exact source RGBA',
								);
								evidence.independentDecoder = decoded;
								evidence.output = {
									bytes: copy.length,
									sha256: sha(copy),
								};
								evidence.checks.push(
									'Actual source-bound v2 Worker preview, explicit metadata choice/revocation/new nonce/Cancel/download, exact private PCND/title/dirty preservation',
								);
								evidence.status = 'passed';
							}
						}
					}
				}
			}
		}
	}
} catch (error) {
	evidence.status = 'failed';
	if (page)
		evidence.workers = await page
			.evaluate(() => window.__metadataWorkers ?? [])
			.catch(() => []);
	evidence.failure = String(error.stack ?? error);
	await page
		?.screenshot({ path: path.join(out, 'failure.png') })
		.catch(() => {});
	throw error;
} finally {
	try {
		for (const [name, hash] of Object.entries(evidence.sources))
			assert.equal(
				sha(fs.readFileSync(path.join(root, name))),
				hash,
				`source changed: ${name}`,
			);
		for (const [name, value] of Object.entries(evidence.assets))
			assert.equal(
				sha(fs.readFileSync(path.join(dist, name))),
				value.sha256,
				`asset changed: ${name}`,
			);
	} catch (error) {
		evidence.status = 'failed';
		evidence.postflightFailure = String(error.stack ?? error);
		process.exitCode = 1;
	}
	evidence.finishedAt = new Date().toISOString();
	fs.writeFileSync(
		path.join(out, 'production.json'),
		`${JSON.stringify(evidence, null, 4)}\n`,
	);
	console.log(JSON.stringify({ out, status: evidence.status, phase }));
	await browser?.close();
	await new Promise((resolve) => server?.close(resolve) ?? resolve());
}
