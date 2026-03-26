import { useState, useCallback } from 'react'
import type { Theme } from '../lib/types'

export function useThemeMode() {
  const [themeMode, setThemeModeState] = useState<Theme>(() => {
    const stored = localStorage.getItem('rtb_theme')
    return (stored === 'light' || stored === 'dark') ? stored : 'dark'
  })

  const toggleTheme = useCallback(() => {
    setThemeModeState(prev => {
      const next = prev === 'dark' ? 'light' : 'dark'
      localStorage.setItem('rtb_theme', next)
      return next
    })
  }, [])

  const setTheme = useCallback((t: Theme) => {
    setThemeModeState(t)
    localStorage.setItem('rtb_theme', t)
  }, [])

  return { themeMode, toggleTheme, setTheme }
}
