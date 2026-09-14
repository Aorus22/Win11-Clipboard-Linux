/**
 * Emoji Search Service
 * Uses Fuse.js for fast fuzzy search across emoji data
 */
import Fuse, { IFuseOptions } from 'fuse.js'
import { loadEmojis, type Emoji } from './emojiService'

/** Fuse.js search options optimized for emoji search */
const FUSE_OPTIONS: IFuseOptions<Emoji> = {
  // Lower threshold = stricter matching (0.0 = exact, 1.0 = match anything)
  threshold: 0.3,
  // Prioritize keywords over name
  keys: [
    { name: 'keywords', weight: 2 },
    { name: 'name', weight: 1 },
  ],
  // Include score for sorting
  includeScore: true,
  // Don't need match info for basic usage
  includeMatches: false,
  // Use extended search for better performance
  useExtendedSearch: false,
  // Minimum characters before search begins
  minMatchCharLength: 1,
  // Where in the word matching should start
  ignoreLocation: true,
}

/** Cached Fuse instance */
let fuseInstance: Fuse<Emoji> | null = null

/**
 * Get or create the Fuse search instance
 * @returns Configured Fuse instance
 */
function getFuseInstance(): Fuse<Emoji> {
  if (!fuseInstance) {
    const emojis = loadEmojis()
    fuseInstance = new Fuse(emojis, FUSE_OPTIONS)
  }
  return fuseInstance
}

/**
 * Search emojis by query
 * @param query Search query string
 * @param limit Maximum results to return (default: 50)
 * @returns Array of matching emojis sorted by relevance
 */
export function searchEmojis(query: string, limit: number = 50): Emoji[] {
  if (!query.trim()) {
    return []
  }

  const fuse = getFuseInstance()
  const results = fuse.search(query, { limit })

  return results.map((result: { item: Emoji }) => result.item)
}

/**
 * Check if the search index is ready
 * @returns true if index is initialized
 */
export function isSearchReady(): boolean {
  return fuseInstance !== null
}

/**
 * Preload the search index (call on app init for faster first search)
 */
export function preloadSearchIndex(): void {
  getFuseInstance()
}
