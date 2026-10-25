import { defineConfig } from 'astro/config';
import wasm from 'vite-plugin-wasm';

export default defineConfig({
	output: 'static',
	devToolbar: { enabled: false },
	vite: {
		plugins: [wasm()],
	},
});
