// A download requests a browser-managed copy; it cannot confirm persistence.
export async function downloadCopy(
	bytes,
	name,
	mime,
	titleText,
	descriptionText,
	linkText,
	cancelText,
) {
	const url = URL.createObjectURL(new Blob([bytes], { type: mime }));
	const dialog = document.createElement('dialog');
	dialog.setAttribute('aria-label', titleText);
	dialog.className = 'paintcraft-download';
	const title = document.createElement('h2');
	title.textContent = titleText;
	const description = document.createElement('p');
	description.textContent = descriptionText;
	const link = document.createElement('a');
	link.href = url;
	link.download = name;
	link.textContent = linkText;
	const cancel = document.createElement('button');
	cancel.textContent = cancelText;
	cancel.className = 'paintcraft-download-cancel';
	dialog.append(title, description, link, cancel);
	document.body.append(dialog);
	try {
		return await new Promise((resolve) => {
			let requested = false;
			link.addEventListener(
				'click',
				() => {
					requested = true;
					// Leave the URL alive while the browser starts consuming the download.
					setTimeout(() => {
						dialog.close();
						resolve(true);
					}, 1000);
				},
				{ once: true },
			);
			cancel.addEventListener('click', () => {
				if (!requested) dialog.close();
			});
			dialog.addEventListener('cancel', (event) => {
				if (requested) event.preventDefault();
			});
			dialog.addEventListener('close', () => resolve(requested), {
				once: true,
			});
			dialog.showModal();
		});
	} finally {
		dialog.remove();
		URL.revokeObjectURL(url);
	}
}
