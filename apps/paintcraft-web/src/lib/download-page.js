import { desktopPlatform, releasesUrl } from './download-platform.js';

const platform = desktopPlatform(navigator);
const status = document.getElementById('download-status');
if (platform) {
	status.textContent = `Opening the latest release for ${platform}… Choose the download for your computer on GitHub.`;
	// Until individual assets are connected, every platform uses the release page.
	window.location.replace(releasesUrl);
} else {
	status.textContent =
		'Choose your desktop platform below. PaintCraft Web is available on this device.';
}
