export const releasesUrl =
	'https://github.com/replicraft/paintcraft/releases/latest';

/** Mobile browsers get manual choices, including iPads reporting a Mac platform. */
export function desktopPlatform(navigator) {
	const agent = navigator.userAgent || '';
	const platform =
		navigator.userAgentData?.platform || navigator.platform || '';
	if (
		navigator.userAgentData?.mobile ||
		/Android|iPhone|iPad|iPod/i.test(agent) ||
		(/Mac/i.test(platform) && navigator.maxTouchPoints > 1)
	) {
		return null;
	}
	const identity = `${platform} ${agent}`;
	if (/Windows|Win32|Win64/i.test(identity)) return 'Windows';
	if (/Mac|Darwin/i.test(identity)) return 'macOS';
	if (/FreeBSD/i.test(identity)) return 'FreeBSD';
	if (/Linux|X11/i.test(identity)) return 'Linux';
	return null;
}
