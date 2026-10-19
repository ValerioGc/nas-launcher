/**
 * Defines types used in the global search application.
 */
export type FileType = 'folder' | 'document' | 'image' | 'audio' | 'video' | 'archive';

/**
 * What a search looks through.
 */
export type SearchScope = 'all' | 'apps' | 'settings' | 'files';

/**
 * Which applications a search includes.
 */
export type AppOrigin = '' | 'system' | 'installed';

/**
 * An application that answers the `search` capability, reached on its own path.
 */
export interface SearchProvider {
  app: string;
  endpoint: string;
  version: number;
}

/**
 * Represents a single search result returned by a search provider.
 * `app_id` and `target` are used by the shell to open the result
 * in the application that produced it.
 */
export interface SearchResult {
  id: string;
  name: string;
  path: string;
  type: FileType;
  extension: string;
  size: number;
  modified: string;
  app_id: string;
  target: string;
}

/**
 * The files found and the providers that did not answer.
 */
export interface FileSearch {
  results: SearchResult[];
  failed: string[];
}

export interface SearchEntry {
  key: string;
  kind: 'app' | 'setting' | 'file';
  title: string;
  detail: string;
  icon?: string;
  app: string;
  query?: string;
  location?: string;
}

/**
 * Defines the columns by which search results can be sorted.
 */
export type SearchSortColumn = 'name' | 'path' | 'type' | 'size' | 'modified';
