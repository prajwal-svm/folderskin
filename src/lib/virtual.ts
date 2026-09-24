/**
 * The arithmetic behind VirtualGrid: how many columns fit, how tall the whole grid is, and which
 * rows are on screen. Pure, so it is tested without a DOM.
 */

export type GridLayout = {
  /** Columns that fit, at least one. */
  columns: number;
  /** Width of one cell once the columns share the row. */
  cellWidth: number;
  /** Height of one row of cells, gap not included. */
  rowHeight: number;
  gap: number;
  rows: number;
  /** Height of the whole grid, for the spacer that gives the scroll bar its length. */
  height: number;
};

/**
 * Lays `count` cells at least `minCell` wide into `width`, `gap` apart. `rowHeight` gets the
 * cell's final width, for cells whose height follows their width (a folder tile, a pack card).
 */
export function gridLayout(
  width: number,
  count: number,
  minCell: number,
  gap: number,
  rowHeight: (cellWidth: number) => number,
): GridLayout {
  const w = Math.max(0, width);
  const columns = Math.max(1, Math.floor((w + gap) / (minCell + gap)));
  const cellWidth = columns === 1 ? w : (w - gap * (columns - 1)) / columns;
  const rh = Math.max(1, rowHeight(cellWidth));
  const rows = Math.ceil(Math.max(0, count) / columns);
  const height = rows === 0 ? 0 : rows * rh + (rows - 1) * gap;
  return { columns, cellWidth, rowHeight: rh, gap, rows, height };
}

/** The items from `start` up to (not including) `end` are drawn; `overscan` rows either side. */
export function visibleRange(
  layout: GridLayout,
  count: number,
  scrollTop: number,
  viewport: number,
  overscan = 2,
): { start: number; end: number } {
  if (count <= 0 || layout.rows === 0) return { start: 0, end: 0 };
  const stride = layout.rowHeight + layout.gap;
  const lastRow = layout.rows - 1;
  // Scrolled past the end (the list just got shorter under a stale scrollTop): keep the last rows.
  const first = Math.min(lastRow, Math.max(0, Math.floor(Math.max(0, scrollTop) / stride) - overscan));
  const last = Math.min(lastRow, Math.floor((Math.max(0, scrollTop) + Math.max(0, viewport)) / stride) + overscan);
  return { start: first * layout.columns, end: Math.min(count, (last + 1) * layout.columns) };
}

/** Where cell `index` sits, relative to the top left of the grid. */
export function cellPosition(layout: GridLayout, index: number): { x: number; y: number } {
  const row = Math.floor(index / layout.columns);
  const col = index % layout.columns;
  return { x: col * (layout.cellWidth + layout.gap), y: row * (layout.rowHeight + layout.gap) };
}

/**
 * The scrollTop that brings cell `index` fully into a viewport `viewport` tall scrolled to
 * `scrollTop`, or `scrollTop` itself when it is already in view. For keyboard navigation.
 */
export function scrollToCell(layout: GridLayout, index: number, scrollTop: number, viewport: number): number {
  const { y } = cellPosition(layout, index);
  if (y < scrollTop) return y;
  const bottom = y + layout.rowHeight;
  if (bottom > scrollTop + viewport) return Math.max(0, bottom - viewport);
  return scrollTop;
}
