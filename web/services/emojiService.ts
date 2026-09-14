/**
 * Emoji Service
 * Loads emoji data from emojilib and provides a clean interface for the application
 */
import emojilib from 'emojilib'

/** Lightweight emoji interface for the app */
export interface Emoji {
  char: string
  name: string
  keywords: string[]
  category: string
}

/** Category mapping for emojis based on keywords */
const CATEGORY_KEYWORDS: Record<string, string[]> = {
  'Smileys & People': [
    'face',
    'smile',
    'laugh',
    'cry',
    'person',
    'people',
    'hand',
    'body',
    'gesture',
    'emotion',
    'heart',
    'love',
    'family',
    'couple',
    'woman',
    'man',
    'girl',
    'boy',
  ],
  'Animals & Nature': [
    'animal',
    'bird',
    'cat',
    'dog',
    'nature',
    'plant',
    'flower',
    'tree',
    'weather',
    'sun',
    'moon',
    'fish',
    'insect',
    'bug',
  ],
  'Food & Drink': [
    'food',
    'fruit',
    'vegetable',
    'drink',
    'meal',
    'eat',
    'beverage',
    'coffee',
    'alcohol',
    'meat',
    'dessert',
  ],
  Activities: ['sport', 'game', 'activity', 'ball', 'music', 'art', 'hobby', 'play', 'exercise'],
  'Travel & Places': [
    'travel',
    'place',
    'vehicle',
    'car',
    'plane',
    'building',
    'city',
    'country',
    'flag',
    'transport',
  ],
  Objects: [
    'object',
    'tool',
    'phone',
    'computer',
    'office',
    'book',
    'money',
    'mail',
    'clothing',
    'fashion',
  ],
  Symbols: ['symbol', 'arrow', 'sign', 'number', 'letter', 'zodiac', 'warning', 'mark', 'button'],
  Flags: ['flag'],
}

/** Detect category based on keywords */
function detectCategory(keywords: string[]): string {
  const keywordSet = new Set(keywords.map((k) => k.toLowerCase()))

  for (const [category, categoryKeywords] of Object.entries(CATEGORY_KEYWORDS)) {
    for (const kw of categoryKeywords) {
      if (keywordSet.has(kw) || keywords.some((k) => k.toLowerCase().includes(kw))) {
        return category
      }
    }
  }

  return 'Other'
}

/** Cached emoji list */
let cachedEmojis: Emoji[] | null = null

/**
 * Load and transform emoji data from emojilib
 * @returns Array of Emoji objects ready for use
 */
export function loadEmojis(): Emoji[] {
  if (cachedEmojis) {
    return cachedEmojis
  }

  const emojis: Emoji[] = []

  // emojilib exports an object where keys are emoji chars and values are keyword arrays
  for (const [char, keywords] of Object.entries(emojilib)) {
    if (!keywords || !Array.isArray(keywords) || keywords.length === 0) {
      continue
    }

    // First keyword is typically the name
    const name = keywords[0].split('_').join(' ')
    const category = detectCategory(keywords)

    emojis.push({
      char,
      name,
      keywords,
      category,
    })
  }

  cachedEmojis = emojis
  return emojis
}

/**
 * Get all unique categories from the emoji list
 * @returns Sorted array of category names
 */
export function getCategories(): string[] {
  const emojis = loadEmojis()
  const categories = new Set(emojis.map((e) => e.category))
  return Array.from(categories).sort((a, b) => a.localeCompare(b))
}

/**
 * Get emojis by category
 * @param category Category name to filter by
 * @returns Array of emojis in that category
 */
export function getEmojisByCategory(category: string): Emoji[] {
  return loadEmojis().filter((e) => e.category === category)
}
