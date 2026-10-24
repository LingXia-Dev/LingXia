export async function openFeedback() {
  await lx.surface.openPage('feedback', {
    as: 'float',
    position: 'bottom',
    size: { width: '100%', height: '80%' },
    interaction: { closeButton: true, dismiss: 'manual', modal: true },
  });
}
