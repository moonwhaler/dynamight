/** Rows to render for a scrolled list with fixed row height. */
export interface VirtualWindow {
  /** First row index to render */
  start: number;
  /** One past the last row index to render */
  end: number;
  /** Space above the rendered rows, in px */
  padTop: number;
  /** Space below the rendered rows, in px */
  padBottom: number;
}

export function virtualWindow(
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  count: number,
  overscan = 20,
): VirtualWindow {
  if (count <= 0 || rowHeight <= 0) return { start: 0, end: 0, padTop: 0, padBottom: 0 };
  const first = Math.floor(Math.max(0, scrollTop) / rowHeight);
  const visible = Math.ceil(Math.max(0, viewportHeight) / rowHeight) + 1;
  const start = Math.max(0, Math.min(count, first - overscan));
  const end = Math.max(start, Math.min(count, first + visible + overscan));
  return { start, end, padTop: start * rowHeight, padBottom: (count - end) * rowHeight };
}
