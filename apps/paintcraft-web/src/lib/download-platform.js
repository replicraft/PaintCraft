export const releasesUrl =
	'https://github.com/replicraft/paintcraft/releases/latest';
export const releasesApi =
	'https://api.github.com/repos/replicraft/paintcraft/releases/latest';

export const platforms = [
	{
		name: 'Windows',
		id: 'windows',
		architectures: [
			{ id: 'x64', label: 'x64', detail: 'Intel and AMD PCs' },
			{ id: 'arm64', label: 'ARM64', detail: 'Windows on ARM' },
		],
		formats: [
			{ suffix: '.msi', label: 'Installer', extension: 'MSI' },
			{ suffix: '-portable.zip', label: 'Portable', extension: 'ZIP' },
			{ suffix: '.exe', label: 'Standalone', extension: 'EXE' },
		],
		cli: '.zip',
	},
	{
		name: 'macOS',
		id: 'macos',
		architectures: [
			{ id: 'arm64', label: 'Apple silicon', detail: 'M1 and newer' },
			{ id: 'x64', label: 'Intel', detail: 'Intel Macs' },
		],
		formats: [
			{ suffix: '.dmg', label: 'Disk image', extension: 'DMG' },
			{
				suffix: '-update.zip',
				label: 'Update archive',
				extension: 'ZIP',
			},
		],
		cli: '.tar.gz',
	},
	{
		name: 'Linux',
		id: 'linux',
		architectures: [
			{ id: 'x64', label: 'x64', detail: 'Intel and AMD PCs' },
			{ id: 'arm64', label: 'ARM64', detail: '64-bit ARM computers' },
		],
		formats: [
			{ suffix: '.AppImage', label: 'AppImage', extension: 'AppImage' },
			{ suffix: '.deb', label: 'Debian / Ubuntu', extension: 'DEB' },
			{ suffix: '.rpm', label: 'Fedora / openSUSE', extension: 'RPM' },
			{
				suffix: '.tar.gz',
				label: 'Portable archive',
				extension: 'tar.gz',
			},
		],
		cli: '.tar.gz',
	},
];

export function assetName(version, target, suffix, cli = false) {
	return `paintcraft${cli ? '-cli' : ''}-${version}-${target}${suffix}`;
}

export function assetUrl(tag, name) {
	return `https://github.com/replicraft/paintcraft/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`;
}

export function desktopPlatform(navigator) {
	const agent = navigator.userAgent || '';
	const platform =
		navigator.userAgentData?.platform || navigator.platform || '';
	if (
		navigator.userAgentData?.mobile ||
		/Android|iPhone|iPad|iPod/i.test(agent) ||
		(/Mac/i.test(platform) && navigator.maxTouchPoints > 1)
	)
		return null;
	const identity = `${platform} ${agent}`;
	if (/Windows|Win32|Win64/i.test(identity)) return 'Windows';
	if (/Mac|Darwin/i.test(identity)) return 'macOS';
	if (/FreeBSD|OpenBSD|NetBSD/i.test(identity)) return null;
	if (/Linux|X11/i.test(identity)) return 'Linux';
	return null;
}

export async function downloadTarget(navigator, macArchitecture = () => null) {
	const platform = desktopPlatform(navigator);
	if (!platform) return null;
	let hints = {};
	let timeout;
	try {
		const request = navigator.userAgentData?.getHighEntropyValues?.([
			'architecture',
			'bitness',
		]);
		if (request)
			hints = await Promise.race([
				request,
				new Promise((resolve) => {
					timeout = setTimeout(() => resolve({}), 1000);
				}),
			]);
	} catch {
		/* User-agent hints are optional. */
	} finally {
		clearTimeout(timeout);
	}
	const identity = `${hints.architecture || ''} ${navigator.platform || ''} ${navigator.userAgent || ''}`;
	if (/armv[5-7]|i[3-6]86/i.test(identity) && !/x86_64|WOW64/i.test(identity))
		return null;
	let architecture = /aarch64|arm64|armv8|\barm\b/i.test(identity)
		? 'arm64'
		: 'x64';
	if (
		platform === 'macOS' &&
		!hints.architecture &&
		!/arm64|aarch64/i.test(identity)
	)
		architecture = macArchitecture() || 'x64';
	const definition = platforms.find((entry) => entry.name === platform);
	return {
		id: `${definition.id}-${architecture}`,
		platform,
		architecture,
		suffix: definition.formats[0].suffix,
	};
}

/** Safari doesn't expose CPU hints; use its renderer when it identifies the Mac chip. */
export function macArchitecture(document) {
	let context;
	try {
		context = document.createElement('canvas').getContext('webgl');
		const extension = context?.getExtension('WEBGL_debug_renderer_info');
		const renderer = extension
			? context.getParameter(extension.UNMASKED_RENDERER_WEBGL)
			: '';
		if (/Apple (?:M\d|GPU)/i.test(renderer)) return 'arm64';
		if (/Intel|AMD|NVIDIA/i.test(renderer)) return 'x64';
	} catch {
		/* Graphics information can be unavailable. */
	} finally {
		context?.getExtension('WEBGL_lose_context')?.loseContext();
	}
	return null;
}

export function releaseAssets(release) {
	if (
		!/^v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(
			release.tag_name || '',
		) ||
		!Array.isArray(release.assets)
	)
		throw new Error('Invalid release metadata');
	const tag = release.tag_name;
	const version = tag.replace(/^v/, '');
	const assets = new Map(
		release.assets
			.filter((asset) => asset.size > 0 && asset.state === 'uploaded')
			.map((asset) => [asset.name, assetUrl(tag, asset.name)]),
	);
	if (assets.size === 0) throw new Error('Release has no downloads');
	return { tag, version, assets };
}
