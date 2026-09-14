/**
 * Type declarations for emojilib
 * @see https://github.com/muan/emojilib
 */
declare module 'emojilib' {
  /**
   * The emojilib object maps emoji characters to arrays of keywords
   * Key: emoji character (e.g., "😀")
   * Value: array of keywords describing the emoji (e.g., ["grinning", "face", "smile"])
   */
  const emojilib: Record<string, string[]>
  export default emojilib
}
