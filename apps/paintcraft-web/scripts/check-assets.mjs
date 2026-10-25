import { fileURLToPath } from 'node:url';
import { checkAssets, ASSET_LIMIT } from './assets.mjs';
const sizes = checkAssets(fileURLToPath(new URL('../dist', import.meta.url)));
console.log(
	`Built ${Object.keys(sizes).length} assets; largest: ${Math.max(...Object.values(sizes)).toLocaleString()} bytes (limit ${ASSET_LIMIT.toLocaleString()})`,
);
