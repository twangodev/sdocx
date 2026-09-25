interface IterableDirectory extends FileSystemDirectoryHandle {
	entries(): AsyncIterableIterator<[string, FileSystemHandle]>;
}

export interface AssetStore {
	read(name: string): Promise<File>;
	write(name: string, contents: Blob): Promise<void>;
	remove(name: string): Promise<void>;
	list(): Promise<string[]>;
}

export class OpfsAssetStore implements AssetStore {
	private async directory(kind: string): Promise<FileSystemDirectoryHandle> {
		const root = await navigator.storage.getDirectory();
		const library = await root.getDirectoryHandle('sdocx', { create: true });
		return library.getDirectoryHandle(kind, { create: true });
	}

	async read(name: string): Promise<File> {
		const [kind, filename] = name.split('/');
		const directory = await this.directory(kind);
		return (await directory.getFileHandle(filename)).getFile();
	}

	async write(name: string, contents: Blob): Promise<void> {
		const [kind, filename] = name.split('/');
		const directory = await this.directory(kind);
		const handle = await directory.getFileHandle(filename, { create: true });
		const stream = await handle.createWritable();
		try {
			await stream.write(contents);
			await stream.close();
		} catch (error) {
			await stream.abort().catch(() => {});
			throw error;
		}
	}

	async remove(name: string): Promise<void> {
		const [kind, filename] = name.split('/');
		try {
			await (await this.directory(kind)).removeEntry(filename);
		} catch (error) {
			if (!(error instanceof DOMException && error.name === 'NotFoundError')) throw error;
		}
	}

	async list(): Promise<string[]> {
		const names: string[] = [];
		for (const kind of ['originals', 'thumbnails']) {
			const directory = await this.directory(kind);
			for await (const [name, handle] of (directory as IterableDirectory).entries()) {
				if (handle.kind === 'file') names.push(`${kind}/${name}`);
			}
		}
		return names;
	}
}
