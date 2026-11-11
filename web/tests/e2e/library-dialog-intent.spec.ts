import { expect, test, type Page } from '../fixtures/browser';
import { pdfNote } from '../fixtures/pdf-note';

async function savedCollections(page: Page) {
	return page.evaluate(() => new Promise<{
		collections: { id: string; name: string }[];
		memberships: { collectionId: string; documentId: string }[];
	}>((resolve, reject) => {
		const opening = indexedDB.open('sdocx-library');
		opening.onerror = () => reject(opening.error);
		opening.onsuccess = () => {
			const database = opening.result;
			const transaction = database.transaction(['collections', 'memberships'], 'readonly');
			const collections = transaction.objectStore('collections').getAll();
			const memberships = transaction.objectStore('memberships').getAll();
			transaction.oncomplete = () => {
				database.close();
				resolve({ collections: collections.result, memberships: memberships.result });
			};
			transaction.onabort = () => { database.close(); reject(transaction.error); };
		};
	}));
}

test('retrying collection membership assignment preserves one collection and its draft', async ({ page }) => {
	await page.route('https://rybbit.twango.dev/**', (route) => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('button', { name: 'Import notes', exact: true })).toBeEnabled();
	const note = { name: 'retry-note.sdocx', mimeType: 'application/zip', buffer: pdfNote() };
	await page.locator('input[type=file]').setInputFiles([note, { ...note, name: 'duplicate.sdocx' }]);
	await expect(page.locator('article.note')).toHaveCount(1);
	await expect(page.getByRole('button', { name: 'Cancel import' })).toHaveCount(0);
	await page.locator('article.note').getByRole('checkbox').check();
	await page.getByRole('button', { name: 'Add selected notes to collection' }).click();
	await page.getByRole('menuitem', { name: 'New collection…', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'Create collection', exact: true });
	await dialog.getByRole('textbox', { name: 'Collection name' }).fill('Course');
	await page.evaluate(() => {
		const put = IDBObjectStore.prototype.put;
		let failNextMembership = true;
		IDBObjectStore.prototype.put = function (...args) {
			if (this.name === 'memberships' && failNextMembership) {
				failNextMembership = false;
				throw new DOMException('Membership storage temporarily unavailable', 'QuotaExceededError');
			}
			return put.apply(this, args);
		};
	});
	await dialog.getByRole('button', { name: 'Save collection', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText('Membership storage temporarily unavailable');
	await expect(dialog.getByRole('textbox', { name: 'Collection name' })).toHaveValue('Course');
	const firstAttempt = await savedCollections(page);
	expect(firstAttempt.collections).toHaveLength(1);
	expect(firstAttempt.memberships).toHaveLength(0);
	await dialog.getByRole('button', { name: 'Save collection', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	const retried = await savedCollections(page);
	expect(retried.collections).toEqual(firstAttempt.collections);
	expect(retried.memberships).toEqual([{ collectionId: firstAttempt.collections[0].id, documentId: expect.any(String) }]);
	await page.getByRole('button', { name: 'Clear selection', exact: true }).click();
	await page.getByRole('navigation', { name: 'Collections', exact: true }).getByRole('button', { name: /Course/ }).click();
	await expect(page.locator('article.note')).toHaveCount(1);
});
