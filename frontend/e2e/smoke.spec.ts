import { test, expect } from '@playwright/test';

// No wallet is injected in headless Chromium, so the app renders its
// disconnected shell. This smoke test asserts the page boots and the core
// scaffolding (hero, connect affordance, draft form) is present — i.e. the
// bundle compiles and the client components hydrate without throwing.
test('renders the disconnected shell', async ({ page }) => {
  await page.goto('/');

  await expect(page.getByRole('heading', { name: /author and submit referenda/i })).toBeVisible();
  await expect(page.getByRole('button', { name: /connect wallet/i })).toBeVisible();

  // The propose form is present but its submit stays disabled until a wallet is
  // connected and the inputs validate.
  const create = page.getByRole('button', { name: /create proposal/i });
  await expect(create).toBeVisible();
  await expect(create).toBeDisabled();

  // A disconnected visitor is told to connect before drafting.
  await expect(page.getByText(/connect your wallet to create a proposal/i)).toBeVisible();
});
