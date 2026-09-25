export async function createThumbnail(svg: string): Promise<Blob> {
	const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
	const image = new Image();
	try {
		await new Promise<void>((resolve, reject) => {
			image.onload = () => resolve();
			image.onerror = () => reject(new Error('Unable to render thumbnail.'));
			image.src = url;
		});
		const scale = Math.min(1, 384 / Math.max(image.naturalWidth, image.naturalHeight));
		const canvas = document.createElement('canvas');
		canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
		canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
		const context = canvas.getContext('2d');
		if (!context) throw new Error('Thumbnail rendering is unavailable.');
		context.drawImage(image, 0, 0, canvas.width, canvas.height);
		return await new Promise<Blob>((resolve, reject) => {
			canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error('Unable to encode thumbnail.')), 'image/png');
		});
	} finally {
		URL.revokeObjectURL(url);
	}
}
