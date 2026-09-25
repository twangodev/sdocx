import { test as base } from '@playwright/test';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export { expect } from '@playwright/test';
export type { Locator, Page } from '@playwright/test';

export const test = base.extend({
	context: async (
		{
			browserName,
			context,
			playwright,
			contextOptions,
			baseURL,
			viewport,
			deviceScaleFactor,
			isMobile,
			hasTouch,
			userAgent,
			headless,
			launchOptions
		},
		use
	) => {
		if (browserName !== 'webkit') {
			await use(context);
			return;
		}
		// WebKit OPFS requires a persistent context: microsoft/playwright#18235.
		const profile = await mkdtemp(join(tmpdir(), 'sdocx-webkit-'));
		try {
			const persistent = await playwright.webkit.launchPersistentContext(profile, {
				...launchOptions,
				...contextOptions,
				baseURL,
				viewport,
				deviceScaleFactor,
				isMobile,
				hasTouch,
				userAgent,
				headless
			});
			try {
				await use(persistent);
			} finally {
				await persistent.close();
			}
		} finally {
			await rm(profile, { recursive: true, force: true });
		}
	}
});
