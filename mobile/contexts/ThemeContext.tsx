import { createContext, useContext } from 'react';
import { useAppTheme, AppColors, AppThemeMode, DARK_COLORS } from '../hooks/useAppTheme';

interface ThemeContextValue {
  mode: AppThemeMode;
  updateMode: (mode: AppThemeMode) => Promise<void>;
  colors: AppColors;
  isDark: boolean;
}

const ThemeContext = createContext<ThemeContextValue>({
  mode: 'dark',
  updateMode: async () => {},
  colors: DARK_COLORS,
  isDark: true,
});

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const theme = useAppTheme();
  return (
    <ThemeContext.Provider value={theme}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  return useContext(ThemeContext);
}
