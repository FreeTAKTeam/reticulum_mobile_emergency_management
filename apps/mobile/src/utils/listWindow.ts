export const LIST_WINDOW_SIZE = 200;

export interface ListWindowBounds {
  page: number;
  pageCount: number;
  startIndex: number;
  endIndex: number;
}

export function listWindowBounds(
  total: number,
  requestedPage: number,
  pageSize = LIST_WINDOW_SIZE,
): ListWindowBounds {
  const safeTotal = Math.max(0, Math.floor(total));
  const safePageSize = Math.max(1, Math.floor(pageSize));
  const pageCount = Math.max(1, Math.ceil(safeTotal / safePageSize));
  const page = Math.min(Math.max(0, Math.floor(requestedPage)), pageCount - 1);
  const startIndex = page * safePageSize;
  return {
    page,
    pageCount,
    startIndex,
    endIndex: Math.min(safeTotal, startIndex + safePageSize),
  };
}

export function sliceListWindow<T>(
  items: readonly T[],
  requestedPage: number,
  pageSize = LIST_WINDOW_SIZE,
): T[] {
  const bounds = listWindowBounds(items.length, requestedPage, pageSize);
  return items.slice(bounds.startIndex, bounds.endIndex);
}
