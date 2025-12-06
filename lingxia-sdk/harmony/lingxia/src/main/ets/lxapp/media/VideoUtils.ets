// Format milliseconds to MM:SS
export function formatVideoTime(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

// Calculate slider percentage (0-100) from time values
export function calculateSliderValue(currentTime: number, duration: number): number {
  if (duration <= 0) return 0;
  return (currentTime / duration) * 100;
}

// Calculate time from slider percentage
export function calculateTimeFromSlider(sliderValue: number, duration: number): number {
  if (duration <= 0) return 0;
  return Math.round((sliderValue / 100) * duration);
}

