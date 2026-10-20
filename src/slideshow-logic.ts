/** Next index in a looping slideshow; a single image has nowhere to go. */
export function nextSlide(index: number, len: number): number | null {
  if (len < 2) return null;
  return (index + 1) % len;
}
