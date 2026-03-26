import { Chip, Box } from '@mui/material'

interface StatusChipProps {
  label: string
  color: string
  size?: 'small' | 'medium'
}

export function StatusChip({ label, color, size = 'small' }: StatusChipProps) {
  return (
    <Chip
      size={size}
      icon={
        <Box
          sx={{
            width: 6,
            height: 6,
            borderRadius: '50%',
            bgcolor: color,
            boxShadow: `0 0 6px ${color}80`,
          }}
        />
      }
      label={label}
      sx={{
        fontSize: 10,
        fontWeight: 500,
        bgcolor: `${color}18`,
        color: color,
        '& .MuiChip-icon': { ml: 0.5 },
      }}
    />
  )
}
