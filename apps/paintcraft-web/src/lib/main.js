import { createCodecService } from './codec-bridge.js';
import { reportStartupFailure, startOwnedPage } from './page-lifetime.js';
const status = document.getElementById('loading');
let app;
let lifetime;
try {
	const [{ WebHandle, install_cjk_fonts }, { loadCjkFonts }] =
		await Promise.all([
			import('../../pkg/paintcraft_web.js'),
			import('./fonts.js'),
		]);
	const fonts = await loadCjkFonts();
	install_cjk_fonts(fonts[0], fonts[1]);
	app = new WebHandle();
	lifetime = app.owner_lifetime();
	window.paintcraft = app;
	await startOwnedPage({
		app,
		canvas: document.getElementById('paintcraft'),
		lifetime,
		createService: createCodecService,
		status,
	});
} catch (error) {
	reportStartupFailure(app, lifetime, status, error);
}
