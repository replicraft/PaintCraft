import {
	assetName,
	downloadTarget,
	macArchitecture,
	platforms,
	releaseAssets,
	releasesApi,
} from './download-platform.js';

const status = document.getElementById('download-status');
const primary = document.getElementById('recommended-download');
const retry = document.getElementById('retry-download');
let chosenManually = false;
document.addEventListener('click', (event) => {
	if (event.target.closest?.('a[data-download], #recommended-download'))
		chosenManually = true;
});

async function start() {
	retry.hidden = true;
	status.textContent = 'Finding the latest download for your computer…';
	try {
		const [response, target] = await Promise.all([
			fetch(releasesApi, {
				headers: { Accept: 'application/vnd.github+json' },
				signal: AbortSignal.timeout(8000),
			}),
			downloadTarget(navigator, () => macArchitecture(document)),
		]);
		if (!response.ok)
			throw new Error(`Release check failed (${response.status})`);
		const { version, tag, assets } = releaseAssets(await response.json());
		document.getElementById('release-version').textContent = `v${version}`;
		for (const link of document.querySelectorAll('a[data-download]')) {
			const name =
				link.dataset.checksums !== undefined
					? 'SHA256SUMS.txt'
					: assetName(
							version,
							link.dataset.target,
							link.dataset.suffix,
							link.dataset.cli !== undefined,
						);
			const url = assets.get(name);
			if (url) {
				link.href = url;
				link.removeAttribute('aria-disabled');
			} else {
				link.removeAttribute('href');
				link.setAttribute('aria-disabled', 'true');
			}
		}
		const name = target && assetName(version, target.id, target.suffix);
		const url = assets.get(name);
		if (!target || !url) {
			status.textContent =
				'Choose a desktop or command-line download below, or try PaintCraft in your browser.';
			return;
		}
		const platform = platforms.find(
			(entry) => entry.name === target.platform,
		);
		const architecture = platform.architectures.find(
			(entry) => entry.id === target.architecture,
		).label;
		primary.href = url;
		primary.textContent = `Download for ${target.platform} (${architecture})`;
		primary.hidden = false;
		if (!chosenManually) {
			primary.click();
			status.textContent = `Your download for ${target.platform} (${architecture}) is starting. If it doesn't start, use the button below.`;
		} else
			status.textContent = `PaintCraft ${tag} is ready to download. Choose another package below if needed.`;
	} catch {
		status.textContent = `Couldn't check the latest release. You can still download ${document.getElementById('release-version').textContent} below, or try again.`;
		retry.hidden = false;
	}
}

retry.addEventListener('click', () => {
	chosenManually = false;
	void start();
});
void start();
