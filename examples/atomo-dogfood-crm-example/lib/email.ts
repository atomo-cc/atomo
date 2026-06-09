export async function sendEmail(input: { to: string; subject: string; template: string; data?: Record<string, unknown> }) {
  console.log('[email]', input)
}
