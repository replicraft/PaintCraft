// Build the persistent private render binding; no Session/history instances.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { BINDGEN_VERSION } from './assets.mjs';
import { LINEAR_MEMORY_BYTES, memoryLimits } from './codec-memory.mjs';

const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
const args = process.argv.slice(2);
if (args.length && (args.length !== 2 || args[0] !== '--out-dir'))
	throw new Error('Usage: build-render-worker.mjs [--out-dir directory]');
const output = path.resolve(args[1] ?? path.join(app, 'render-worker-pkg'));
process.env.PATH =
	path.join(os.homedir(), '.cargo/bin') + path.delimiter + process.env.PATH;
function run(command, args, capture = false) {
	const result = spawnSync(command, args, {
		cwd: root,
		encoding: 'utf8',
		stdio: capture ? 'pipe' : 'inherit',
	});
	if (result.error) throw result.error;
	if (result.status !== 0)
		throw new Error(`${command} failed: ${result.stderr ?? result.status}`);
	return result.stdout?.trim();
}
if (
	run('wasm-bindgen', ['--version'], true) !==
	`wasm-bindgen ${BINDGEN_VERSION}`
)
	throw new Error(
		'Render binding requires the workspace wasm-bindgen CLI version',
	);
run('cargo', [
	'rustc',
	'--locked',
	'--profile',
	'web-release',
	'--target',
	'wasm32-unknown-unknown',
	'-p',
	'paintcraft-web-render-worker',
	'--lib',
	'--',
	'-C',
	`link-arg=--max-memory=${LINEAR_MEMORY_BYTES}`,
]);
const metadata = JSON.parse(
	run('cargo', ['metadata', '--no-deps', '--format-version', '1'], true),
);
const binary = path.join(
	metadata.target_directory,
	'wasm32-unknown-unknown/web-release/paintcraft_web_render_worker.wasm',
);
memoryLimits(fs.readFileSync(binary));
fs.mkdirSync(output, { recursive: true });
run('wasm-bindgen', [
	binary,
	'--target',
	'web',
	'--remove-name-section',
	'--out-dir',
	output,
]);
const version = metadata.packages.find(
	(entry) => entry.name === 'paintcraft-web-render-worker',
).version;
const wasm = path.join(output, 'paintcraft_web_render_worker_bg.wasm');
const limits = memoryLimits(fs.readFileSync(wasm));
fs.writeFileSync(
	path.join(output, 'render-worker-build.json'),
	JSON.stringify(
		{
			version: metadata.packages.find(
				(entry) => entry.name === 'paintcraft-web-render-worker',
			).version,
			protocol: 1,
			linearMemoryBytes: LINEAR_MEMORY_BYTES,
			...limits,
		},
		null,
		4,
	) + '\n',
);
console.log(
	`Built persistent render Worker: ${fs.statSync(wasm).size.toLocaleString()} bytes, maximum ${limits.maximum} memory pages`,
);
