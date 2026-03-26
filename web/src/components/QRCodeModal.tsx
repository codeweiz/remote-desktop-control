import { useState, useEffect } from 'react'
import { X, Copy, Check } from 'lucide-react'
import QRCode from 'qrcode'
import { getToken } from '../lib/api'

interface QRCodeModalProps {
  isOpen: boolean
  onClose: () => void
}

export function QRCodeModal({ isOpen, onClose }: QRCodeModalProps) {
  const [svgData, setSvgData] = useState<string>('')
  const [copied, setCopied] = useState(false)

  const deepLink = buildDeepLink()

  useEffect(() => {
    if (!isOpen) return

    QRCode.toString(deepLink, {
      type: 'svg',
      color: {
        dark: '#c9d1d9',
        light: '#00000000',
      },
      margin: 1,
      width: 220,
    }).then(svg => {
      setSvgData(svg)
    }).catch(() => {
      setSvgData('')
    })
  }, [isOpen, deepLink])

  const handleCopy = () => {
    navigator.clipboard.writeText(deepLink).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }).catch(() => {
      // Clipboard not available
    })
  }

  if (!isOpen) return null

  return (
    <div
      className="fixed inset-0 z-50 command-palette-overlay flex items-center justify-center"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="w-[320px] max-w-[90vw] bg-bg-secondary border border-border rounded-xl shadow-2xl overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <h2 className="text-sm font-semibold text-text-primary">Connect via QR Code</h2>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-bg-tertiary text-text-secondary hover:text-text-primary transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        {/* QR Code */}
        <div className="flex flex-col items-center p-6">
          {svgData ? (
            <div
              className="bg-bg-tertiary rounded-lg p-4"
              dangerouslySetInnerHTML={{ __html: svgData }}
            />
          ) : (
            <div className="w-[220px] h-[220px] bg-bg-tertiary rounded-lg flex items-center justify-center">
              <span className="text-xs text-text-secondary">Generating...</span>
            </div>
          )}

          <p className="text-[10px] text-text-secondary mt-4 text-center max-w-[240px]">
            Scan this QR code with the RTB mobile app to connect to this server
          </p>

          {/* Deep link URL */}
          <div className="mt-3 w-full">
            <div className="flex items-center gap-1 bg-bg-tertiary rounded-lg px-3 py-2 border border-border">
              <code className="text-[10px] text-text-secondary flex-1 truncate">{deepLink}</code>
              <button
                onClick={handleCopy}
                className="shrink-0 p-1 rounded hover:bg-bg-secondary text-text-secondary hover:text-text-primary transition-colors"
                title="Copy link"
              >
                {copied ? <Check size={12} className="text-accent-green" /> : <Copy size={12} />}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

function buildDeepLink(): string {
  const host = window.location.host
  const token = getToken()
  const params = new URLSearchParams()
  params.set('host', host)
  if (token) {
    params.set('token', token)
  }
  return `rtb://connect?${params.toString()}`
}
