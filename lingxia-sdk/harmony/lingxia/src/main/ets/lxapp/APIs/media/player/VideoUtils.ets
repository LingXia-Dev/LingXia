// Format milliseconds to MM:SS
export function formatVideoTime(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

// Calculate slider percentage (0-100) from time values
export function calculateSliderValue(currentTime: number, duration: number): number {
  if (duration <= 0 || !isFinite(duration) || !isFinite(currentTime)) return 0;
  return Math.min(100, Math.max(0, (currentTime / duration) * 100));
}

// Calculate time from slider percentage
export function calculateTimeFromSlider(sliderValue: number, duration: number): number {
  if (duration <= 0 || !isFinite(duration) || !isFinite(sliderValue)) return 0;
  const clamped = Math.min(100, Math.max(0, sliderValue));
  return Math.round((clamped / 100) * duration);
}
