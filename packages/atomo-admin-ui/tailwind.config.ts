import type { Config } from 'tailwindcss'
import { colors, spacing, borderRadius, typography, shadows } from '../../design-tokens'

const config: Config = {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        primary: colors.primary,
        success: colors.success,
        danger: colors.danger,
        warning: colors.warning,
        gray: colors.gray,
      },
      spacing,
      borderRadius,
      fontFamily: typography.fontFamily,
      fontSize: typography.fontSize,
      boxShadow: {
        ...shadows,
      },
      animation: {
        'fade-in': 'fadeIn 200ms ease-in-out',
        'slide-in': 'slideIn 300ms ease-out',
        'spin-slow': 'spin 2s linear infinite',
      },
      keyframes: {
        fadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
        slideIn: {
          '0%': { transform: 'translateX(-100%)' },
          '100%': { transform: 'translateX(0)' },
        },
      },
    },
  },
  plugins: [],
}

export default config
