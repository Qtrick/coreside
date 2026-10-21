/**
 * Query-backed DataTable mode is selected when server query control keys are
 * wired. Authoritative totalCount may arrive after the first hydration; until
 * then the table shows a loading empty state instead of a false "no data".
 */
export function isQueryBackedDataTable(options: {
  totalCount: number | null | undefined;
  offsetKey?: string | null;
  sortFieldKey?: string | null;
  sortDirKey?: string | null;
  searchKey?: string | null;
}): boolean {
  return Boolean(
    options.offsetKey ||
      options.sortFieldKey ||
      options.sortDirKey ||
      options.searchKey,
  );
}
