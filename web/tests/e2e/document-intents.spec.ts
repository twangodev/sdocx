import { expect, test, type Page } from '../fixtures/browser';
import { pdfNote } from '../fixtures/pdf-note';

const note = { name: 'intent-note.sdocx', mimeType: 'application/zip', buffer: pdfNote() };

test.beforeEach(async ({ page }) => {
	await page.route('https://rybbit.twango.dev/**', (route) => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('button', { name: 'Import notes', exact: true })).toBeEnabled();
});

async function holdOriginalRead(page: Page, failRead = false): Promise<void> {
	await page.evaluate((failRead) => {
		const original = FileSystemFileHandle.prototype.getFile;
		let release!: () => void;
		const gate = new Promise<void>((resolve) => { release = resolve; });
		let released = false;
		Object.assign(window, { intentReadHeld: false, intentReadSettled: false, releaseIntentRead: () => { released = true; release(); } });
		const getAll = IDBObjectStore.prototype.getAll;
		IDBObjectStore.prototype.getAll = function (...args) {
			const result = getAll.apply(this, args);
			if (released && this.name === 'memberships')
				this.transaction.addEventListener('complete', () => Object.assign(window, { intentReadSettled: true }));
			return result;
		};
		FileSystemFileHandle.prototype.getFile = async function () {
			if (this.name.endsWith('.sdocx')) {
				Object.assign(window, { intentReadHeld: true });
				await gate;
				if (failRead) throw new Error('Obsolete original read failure');
			}
			return original.call(this);
		};
	}, failRead);
}

for (const failRead of [false, true]) {
	test(`Home supersedes a saved note whose original read ${failRead ? 'fails' : 'finishes'} later`, async ({ page }) => {
		await page.locator('input[type=file]').setInputFiles([note, { ...note, name: 'duplicate.sdocx' }]);
		await expect(page.locator('article.note')).toHaveCount(1);
		await expect(page.getByRole('button', { name: 'Cancel import' })).toHaveCount(0);
		await holdOriginalRead(page, failRead);
		await page.getByRole('button', { name: /^Open / }).click();
		await expect.poll(() => page.evaluate(() => Reflect.get(window, 'intentReadHeld'))).toBe(true);
		await page.getByRole('link', { name: 'sdocx home' }).click();
		await page.evaluate(async () => {
			Reflect.get(window, 'releaseIntentRead')();
			await navigator.locks.request('sdocx-library', async () => {});
		});
		await expect.poll(() => page.evaluate(() => Reflect.get(window, 'intentReadSettled'))).toBe(true);
		await expect(page.getByRole('region', { name: 'Document converter' })).toHaveCount(0);
		await expect(page.locator('article.note')).toHaveCount(1);
		await expect(page.getByRole('alert')).toHaveCount(0);
	});
}

test('Home keeps a finishing import saved without automatically reopening it', async ({ page }) => {
	await page.evaluate(() => {
		const original = FileSystemFileHandle.prototype.createWritable;
		let release!: () => void;
		const gate = new Promise<void>((resolve) => { release = resolve; });
		let released = false;
		Object.assign(window, { intentSaveHeld: false, intentImportSettled: false, intentAutoOpen: false,
			releaseIntentSave: () => { released = true; release(); } });
		const getAll = IDBObjectStore.prototype.getAll;
		IDBObjectStore.prototype.getAll = function (...args) {
			const result = getAll.apply(this, args);
			if (released && this.name === 'memberships')
				this.transaction.addEventListener('complete', () => Object.assign(window, { intentImportSettled: true }));
			return result;
		};
		const arrayBuffer = File.prototype.arrayBuffer;
		File.prototype.arrayBuffer = function () {
			if (released && this.name === 'intent-note.sdocx') Object.assign(window, { intentAutoOpen: true });
			return arrayBuffer.call(this);
		};
		FileSystemFileHandle.prototype.createWritable = async function (...args) {
			if (this.name.endsWith('.sdocx')) {
				Object.assign(window, { intentSaveHeld: true });
				await gate;
			}
			return original.apply(this, args);
		};
	});
	await page.locator('input[type=file]').setInputFiles(note);
	await expect.poll(() => page.evaluate(() => Reflect.get(window, 'intentSaveHeld'))).toBe(true);
	await page.getByRole('link', { name: 'sdocx home' }).click();
	await page.evaluate(() => Reflect.get(window, 'releaseIntentSave')());
	await expect.poll(() => page.evaluate(() => Reflect.get(window, 'intentImportSettled'))).toBe(true);
	await page.evaluate(() => navigator.locks.request('sdocx-library', async () => {}));
	expect(await page.evaluate(() => Reflect.get(window, 'intentAutoOpen'))).toBe(false);
	await expect(page.getByRole('button', { name: 'Cancel import' })).toHaveCount(0);
	await expect(page.locator('article.note')).toHaveCount(1);
	await expect(page.getByRole('region', { name: 'Document converter' })).toHaveCount(0);
	await page.reload();
	await expect(page.locator('article.note')).toHaveCount(1);
});


test('a temporary note supersedes a saved note with a delayed original read', async ({ page }) => {
	await page.locator('input[type=file]').setInputFiles([note, { ...note, name: 'duplicate.sdocx' }]);
	await expect(page.locator('article.note')).toHaveCount(1);
	await expect(page.getByRole('button', { name: 'Cancel import' })).toHaveCount(0);
	await page.evaluate(() => {
		const original = FileSystemFileHandle.prototype.createWritable;
		Object.assign(window, { restoreIntentWrites: () => { FileSystemFileHandle.prototype.createWritable = original; } });
		FileSystemFileHandle.prototype.createWritable = async function () {
			throw new DOMException('Storage full', 'QuotaExceededError');
		};
	});
	await page.locator('input[type=file]').setInputFiles({ ...note, name: 'temporary.sdocx', buffer: pdfNote({ threePages: true }) });
	await expect(page.getByRole('button', { name: 'Open temporarily' })).toBeVisible();
	await page.evaluate(() => Reflect.get(window, 'restoreIntentWrites')());
	await holdOriginalRead(page);
	await page.getByRole('button', { name: /^Open intent-note/ }).click();
	await expect.poll(() => page.evaluate(() => Reflect.get(window, 'intentReadHeld'))).toBe(true);
	await page.getByRole('button', { name: 'Open temporarily' }).click();
	const converter = page.getByRole('region', { name: 'Document converter' });
	await expect(converter).toContainText('Temporary note · not saved');
	await expect(converter.getByRole('heading', { name: 'temporary.sdocx', exact: true })).toBeVisible();
	await page.evaluate(async () => {
		Reflect.get(window, 'releaseIntentRead')();
		await navigator.locks.request('sdocx-library', async () => {});
	});
	await expect.poll(() => page.evaluate(() => Reflect.get(window, 'intentReadSettled'))).toBe(true);
	await expect(converter).toContainText('Temporary note · not saved');
	await expect(converter.getByRole('heading', { name: 'temporary.sdocx', exact: true })).toBeVisible();
});
