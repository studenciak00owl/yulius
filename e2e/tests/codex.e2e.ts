import { test } from '@e2e-dev/web';
import { expect } from 'e2e';
import { axeViolations, failOnPageErrors, mock } from './support.ts';

failOnPageErrors();

// Codex as a second harness in this fork (SPEC-445 CDX-010): Codex subscriptions in Accounts,
// Codex in the composer's model menu, and the agent of an automation's tasks.

type Mocked = {
  __yuliusMock: {
    tasks: { harness: string }[];
    accounts: { label: string; provider: string; status: string }[];
    automations: { name: string; harness: string }[];
  };
};

test('a Codex subscription is added with an OpenAI API key, without a sign-in', async ({ app, browser, screen }) => {
  await app.open(mock('/admin/accounts', '&ambient=0'));
  await screen.getByRole('button', 'Add subscription').first().click();
  await expect(screen.getByRole('dialog', 'Add a Claude subscription')).toBeVisible();
  // A segmented choice: its label is what is pressed.
  await browser.locator('dialog.modal .seg label:has-text("Codex")').click();
  const add = screen.getByRole('dialog', 'Add a Codex subscription');
  await expect(add.getByRole('radio', 'ChatGPT sign-in')).toBeChecked();
  await browser.locator('dialog.modal .seg label:has-text("API key")').click();
  await expect(add.getByRole('radio', 'API key')).toBeChecked();
  await add.getByRole('textbox', /^Name/).fill('Codex key');
  expect(await axeViolations(browser)).toEqual([]);
  await add.getByRole('button', 'Add subscription').click();
  await expect(add.getByText('Paste the OpenAI API key.')).toBeVisible();
  await browser.locator('dialog.modal input[type=password]').fill('sk-proj-SYNTHETIC0123456789abcdefghij');
  await add.getByRole('button', 'Add subscription').click();
  await expect(add).toBeHidden();
  await expect(browser.locator('.toast-ok .toast-msg:has-text("Codex key is connected")')).toBeVisible();
  await expect(browser.locator('main .card:has-text("Codex key")')).toBeVisible();
  const added = await browser.evaluate(() => (window as unknown as Mocked).__yuliusMock.accounts.find((a) => a.label === 'Codex key'));
  expect(added).toMatchObject({ provider: 'openai', status: 'ok' });
});

test('choosing Codex in the composer starts a Codex task', async ({ app, browser, screen }) => {
  await app.open(mock('/s/studio'));
  // A Codex task needs a Codex subscription (SPEC-246 CLA-002, SPEC-445).
  await browser.evaluate(() => (window as unknown as Mocked).__yuliusMock.accounts.push({ id: 'acc_codex', provider: 'openai', label: 'Codex #1', status: 'ok' } as never));
  await screen.getByRole('button', /^Model:/).click();
  await screen.getByRole('menuitemradio', /^Codex/).click();
  await expect(screen.getByRole('button', 'Model: Codex')).toBeVisible();
  const prompt = screen.getByRole('textbox', 'Task prompt');
  await prompt.fill('Add a health check');
  await prompt.press('ControlOrMeta+Enter');
  await expect(browser).toHaveURL(/#\/s\/studio\/t\/\d+$/);
  const harness = await browser.evaluate(() => {
    const tasks = (window as unknown as Mocked).__yuliusMock.tasks;
    return tasks[tasks.length - 1].harness;
  });
  expect(harness).toBe('codex');
});

test('an automation chooses the agent of its tasks', async ({ app, browser, screen }) => {
  await app.open(mock('/s/studio/automations/new'));
  await screen.getByRole('textbox', 'Automation name').fill('Nightly Codex review');
  await screen.getByRole('textbox', 'What the task does').fill('Review yesterday’s commits.');
  const agent = screen.getByRole('radiogroup', 'Agent');
  await expect(agent.getByRole('radio', 'Claude Code')).toHaveAttribute('aria-checked', 'true');
  await agent.getByRole('radio', 'Codex').click();
  await expect(agent.getByRole('radio', 'Codex')).toHaveAttribute('aria-checked', 'true');
  expect(await axeViolations(browser)).toEqual([]);
  await screen.getByRole('button', 'Create automation').click();
  await expect.poll(() => browser.evaluate(() => (window as unknown as Mocked).__yuliusMock.automations.find((a) => a.name === 'Nightly Codex review')?.harness)).toBe('codex');
});
