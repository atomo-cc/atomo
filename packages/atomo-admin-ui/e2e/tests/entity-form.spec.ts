import { test, expect } from '@playwright/test'

// The demo Contact schema declares `avatar: File`, so the dynamic form must auto-render the
// MediaUploader for it (the file-typed FormField case wired to /media).
test('Contact form auto-renders the media uploader for the File field', async ({ page }) => {
  await page.goto('/entities/Contact/new')
  await expect(page.getByTestId('media-uploader-dropzone')).toBeVisible({ timeout: 15_000 })
})
