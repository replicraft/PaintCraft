// The build copies the shared root font assets and emits this hashed manifest.
export async function loadCjkFonts(base = new URL(import.meta.env.BASE_URL, window.location.href)) {
  const manifestUrl = new URL('fonts/manifest.json', base);
  const response = await fetch(manifestUrl);
  if (!response.ok) throw new Error(`Font manifest: HTTP ${response.status}`);
  const manifest = await response.json();
  return Promise.all(['japanese', 'chinese'].map(async (name) => {
    const font = manifest[name];
    if (!font || !Number.isSafeInteger(font.bytes) || font.bytes < 32 || font.bytes > 16 * 1024 * 1024
      || !/^[a-f0-9]{64}$/.test(font.sha256)
      || !/^[a-zA-Z0-9.-]+\.otf$/.test(font.file)) throw new Error(`Invalid ${name} font manifest`);
    const response = await fetch(new URL(font.file, manifestUrl));
    if (!response.ok) throw new Error(`${name} font: HTTP ${response.status}`);
    const data = await response.arrayBuffer();
    if (data.byteLength !== font.bytes) throw new Error(`${name} font size mismatch`);
    const digest = await crypto.subtle.digest('SHA-256', data);
    const hex = [...new Uint8Array(digest)].map(byte => byte.toString(16).padStart(2, '0')).join('');
    if (hex !== font.sha256) throw new Error(`${name} font checksum mismatch`);
    return new Uint8Array(data);
  }));
}
