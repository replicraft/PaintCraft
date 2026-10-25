const status = document.getElementById('loading');
let app;
try {
	const [{ WebHandle, install_cjk_fonts }, { loadCjkFonts }] =
		await Promise.all([
			import('../../pkg/paintcraft_web.js'),
			import('./fonts.js'),
		]);
	const fonts = await loadCjkFonts();
	install_cjk_fonts(fonts[0], fonts[1]);
	app = new WebHandle();
	window.paintcraft = app;
	await app.start(document.getElementById('paintcraft'));
	status.hidden = true;
	window.addEventListener('beforeunload', (event) => {
		if (app.is_dirty()) {
			event.preventDefault();
			event.returnValue = '';
		}
	});
	window.addEventListener('pagehide', (event) => {
		if (!event.persisted) app.destroy();
	});
} catch (error) {
	app?.destroy();
	status.textContent = `PaintCraft could not start. PaintCraft needs WebAssembly and WebGPU or WebGL. ${error}`;
	console.error(error);
}
