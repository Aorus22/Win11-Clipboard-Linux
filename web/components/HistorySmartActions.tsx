import { clsx } from 'clsx'
import { ExternalLink, Mail } from 'lucide-react'
import type { SmartAction } from '../services/smartActionService'

export function HistorySmartActions({
  linkAction,
  emailAction,
  isDark,
  onActionClick,
}: {
  linkAction?: SmartAction
  emailAction?: SmartAction
  isDark: boolean
  onActionClick: (e: React.MouseEvent, action: SmartAction) => void
}) {
  if (!linkAction && !emailAction) return null

  const buttonClasses = (isDark: boolean) =>
    clsx(
      'p-1.5 rounded-md transition-colors',
      isDark
        ? 'text-win11-text-tertiary hover:bg-win11-bg-tertiary'
        : 'text-win11Light-text-secondary hover:bg-win11Light-bg-tertiary'
    )

  return (
    <>
      {linkAction && (
        <button
          onClick={(e) => onActionClick(e, linkAction)}
          className={buttonClasses(isDark)}
          title="Open Link"
          tabIndex={-1}
        >
          <ExternalLink className="w-4 h-4" />
        </button>
      )}
      {emailAction && (
        <button
          onClick={(e) => onActionClick(e, emailAction)}
          className={buttonClasses(isDark)}
          title="Compose Email"
          tabIndex={-1}
        >
          <Mail className="w-4 h-4" />
        </button>
      )}
    </>
  )
}
