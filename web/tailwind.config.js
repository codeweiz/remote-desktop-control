/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        bg: {
          primary: 'var(--bg-primary)',
          secondary: 'var(--bg-secondary)',
          tertiary: 'var(--bg-tertiary)',
        },
        border: {
          DEFAULT: 'var(--border-color)',
        },
        text: {
          primary: 'var(--text-primary)',
          secondary: 'var(--text-secondary)',
        },
        accent: {
          green: 'var(--accent-green)',
          blue: 'var(--accent-blue)',
          purple: 'var(--accent-purple)',
          orange: 'var(--accent-orange)',
          red: 'var(--accent-red)',
        },
      },
    },
  },
  plugins: [],
}
