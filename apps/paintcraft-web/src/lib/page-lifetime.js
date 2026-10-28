// Host callbacks belong to the public owner, not a particular runner restart.
export async function startOwnedPage({
	app,
	canvas,
	lifetime,
	createService,
	status,
	target = window,
}) {
	try {
		await app.start(canvas);
	} catch (error) {
		if (lifetime.aborted) return 'disposed';
		throw error;
	}
	if (lifetime.aborted) return 'disposed';
	app.install_codec_service(createService(app.application_version()));
	status.hidden = true;
	target.addEventListener(
		'beforeunload',
		(event) => {
			if (app.is_dirty()) {
				event.preventDefault();
				event.returnValue = '';
			}
		},
		{ signal: lifetime },
	);
	target.addEventListener(
		'pagehide',
		(event) => {
			if (!event.persisted) app.destroy();
		},
		{ signal: lifetime },
	);
	return 'ready';
}

export function reportStartupFailure(
	app,
	lifetime,
	status,
	error,
	logger = console,
) {
	if (lifetime?.aborted) return 'disposed';
	app?.destroy();
	status.textContent = `PaintCraft could not start. PaintCraft needs WebAssembly and WebGPU or WebGL. ${error}`;
	logger.error(error);
	return 'failed';
}
