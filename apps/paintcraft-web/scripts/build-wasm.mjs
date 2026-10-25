// Compile Rust once; Astro/Vite own the website and Wasm module bundling.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { copyFonts, BINDGEN_VERSION as version } from './assets.mjs';

const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
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
		throw new Error(
			`${command} failed (${result.status}): ${result.stderr ?? ''}`,
		);
	return result.stdout?.trim();
}
if (
	spawnSync('rustup', ['--version'], { stdio: 'ignore' }).error?.code ===
	'ENOENT'
) {
	run('sh', ['scripts/bootstrap_web_rust.sh']);
}
run('rustup', ['target', 'add', 'wasm32-unknown-unknown']);
const installed = spawnSync('wasm-bindgen', ['--version'], {
	encoding: 'utf8',
});
if (
	installed.status !== 0 ||
	installed.stdout.trim() !== `wasm-bindgen ${version}`
) {
	run('cargo', [
		'install',
		'wasm-bindgen-cli',
		'--version',
		version,
		'--locked',
		'--force',
	]);
}
run('cargo', [
	'build',
	'--locked',
	'--profile',
	'web-release',
	'--target',
	'wasm32-unknown-unknown',
	'-p',
	'paintcraft-web',
]);
const metadata = JSON.parse(
	run('cargo', ['metadata', '--no-deps', '--format-version', '1'], true),
);
const wasm = path.join(
	metadata.target_directory,
	'wasm32-unknown-unknown/web-release/paintcraft_web.wasm',
);
run('wasm-bindgen', [
	wasm,
	'--target',
	'bundler',
	'--remove-name-section',
	'--out-dir',
	path.join(app, 'pkg'),
]);
copyFonts(root, path.join(app, 'public/fonts'));
