import { forwardRef, useCallback, useImperativeHandle, useLayoutEffect, useMemo, useRef, useState, type ReactNode, type Ref } from "react";
import { cellPosition, gridLayout, scrollToCell, visibleRange } from "../lib/virtual";

export type VirtualGridHandle = {
  /** Scrolls just far enough to show item `index`. */
  scrollToIndex: (index: number) => void;
  /** The scrolling element itself. */
  element: () => HTMLDivElement | null;
  /** How many columns fit now, for arrow keys that move a row at a time. */
  columns: () => number;
};

type Props<T> = {
  items: readonly T[];
  /** The narrowest a cell may be; the columns share what's left over. */
  minCell: number;
  gap?: number;
  /** A row's height for a given cell width. */
  rowHeight: (cellWidth: number) => number;
  /** Rows drawn above and below what's on screen. */
  overscan?: number;
  getKey: (item: T, index: number) => string;
  renderItem: (item: T, index: number, cellWidth: number) => ReactNode;
  className?: string;
  /** Space inside the scroll box around the grid, in px. */
  padding?: number;
  role?: string;
  "aria-label"?: string;
  /** Shown instead of the grid when there are no items. */
  empty?: ReactNode;
};

/**
 * A grid that only draws the rows on screen, for lists that can run to thousands of items: every
 * icon in a pack, every community pack. Cells are absolutely placed inside a spacer as tall as the
 * whole grid, so the scroll bar is honest and the page never lays out what it can't show.
 */
function VirtualGridInner<T>(
  { items, minCell, gap = 8, rowHeight, overscan = 2, getKey, renderItem, className, padding = 0, role, empty, ...aria }: Props<T>,
  ref: Ref<VirtualGridHandle>,
) {
  const scroller = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ width: 0, height: 0 });
  const [scrollTop, setScrollTop] = useState(0);
  const frame = useRef(0);

  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const measure = () => setBox({ width: el.clientWidth - padding * 2, height: el.clientHeight });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [padding]);

  useLayoutEffect(() => () => cancelAnimationFrame(frame.current), []);

  const layout = useMemo(() => gridLayout(box.width, items.length, minCell, gap, rowHeight), [box.width, items.length, minCell, gap, rowHeight]);
  const { start, end } = visibleRange(layout, items.length, scrollTop - padding, box.height, overscan);

  const onScroll = useCallback(() => {
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(() => setScrollTop(scroller.current?.scrollTop ?? 0));
  }, []);

  useImperativeHandle(
    ref,
    () => ({
      scrollToIndex: (index) => {
        const el = scroller.current;
        if (!el || index < 0 || index >= items.length) return;
        const top = scrollToCell(layout, index, el.scrollTop - padding, el.clientHeight - padding * 2) + padding;
        if (top !== el.scrollTop) el.scrollTop = top;
      },
      element: () => scroller.current,
      columns: () => layout.columns,
    }),
    [layout, items.length, padding],
  );

  const cells: ReactNode[] = [];
  for (let i = start; i < end; i++) {
    const { x, y } = cellPosition(layout, i);
    cells.push(
      <div
        key={getKey(items[i], i)}
        className="vgrid-cell"
        data-index={i}
        style={{ transform: `translate(${x + padding}px, ${y + padding}px)`, width: layout.cellWidth, height: layout.rowHeight }}
      >
        {renderItem(items[i], i, layout.cellWidth)}
      </div>,
    );
  }

  return (
    <div ref={scroller} className={`vgrid${className ? ` ${className}` : ""}`} onScroll={onScroll} role={role} {...aria}>
      {items.length === 0 && empty ? empty : <div className="vgrid-space" style={{ height: layout.height + padding * 2 }}>{cells}</div>}
    </div>
  );
}

export const VirtualGrid = forwardRef(VirtualGridInner) as <T>(props: Props<T> & { ref?: Ref<VirtualGridHandle> }) => ReturnType<typeof VirtualGridInner>;
