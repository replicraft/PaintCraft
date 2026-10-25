import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';

export const BINDGEN_VERSION = '0.2.129';
export const ASSET_LIMIT = 25 * 1024 * 1024;
export function checkAssets(directory) {
	const sizes = {};
	function visit(dir) {
		for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
			const file = path.join(dir, entry.name);
			if (entry.isDirectory()) visit(file);
			else if (entry.isFile()) {
				const name = path
					.relative(directory, file)
					.split(path.sep)
					.join('/');
				const size = fs.statSync(file).size;
				if (size > ASSET_LIMIT)
					throw new Error(
						`${name} is ${size.toLocaleString()} bytes; Cloudflare Pages allows ${ASSET_LIMIT.toLocaleString()} per asset`,
					);
				sizes[name] = size;
			}
		}
	}
	visit(directory);
	return sizes;
}
export function copyFonts(root, output) {
	fs.rmSync(output, { recursive: true, force: true });
	fs.mkdirSync(output, { recursive: true });
	const manifest = {};
	for (const [name, family, filename] of [
		['japanese', 'noto-sans-jp', 'NotoSansJP-Regular.otf'],
		['chinese', 'noto-sans-sc', 'NotoSansSC-Regular.otf'],
	]) {
		const source = path.join(root, 'assets/fonts', family, filename);
		const data = fs.readFileSync(source);
		const sha256 = createHash('sha256').update(data).digest('hex');
		const file = `${path.parse(filename).name}.${sha256}.otf`;
		fs.copyFileSync(source, path.join(output, file));
		fs.copyFileSync(
			path.join(path.dirname(source), 'LICENSE'),
			path.join(output, `${family}-LICENSE.txt`),
		);
		manifest[name] = { file, sha256, bytes: data.length };
	}
	for (const [family, license] of [
		['selawik', 'LICENSE.txt'],
		['noto-sans-devanagari', 'LICENSE'],
	]) {
		fs.copyFileSync(
			path.join(root, 'assets/fonts', family, license),
			path.join(output, `${family}-LICENSE.txt`),
		);
	}
	fs.writeFileSync(
		path.join(output, 'manifest.json'),
		JSON.stringify(manifest, null, 2) + '\n',
	);
	return manifest;
}
