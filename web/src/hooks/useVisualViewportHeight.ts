import { useState, useEffect } from 'react'

/** Tracks the visual viewport height to detect virtual keyboard open/close. */
export function useVisualViewportHeight(): number {
  const [height, setHeight] = useState(window.visualViewport?.height ?? window.innerHeight)

  useEffect(() => {
    const vv = window.visualViewport
    if (!vv) return

    const onResize = () => setHeight(vv.height)
    vv.addEventListener('resize', onResize)
    return () => vv.removeEventListener('resize', onResize)
  }, [])

  return height
}
