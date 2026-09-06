import type { Config } from 'tailwindcss'
import { colors, spacing, borderRadius, typography, shadows } from '../../design-tokens'

const config: Config = {
  darkMode: 'class',
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        primary: {
          ...colors.primary,
          DEFAULT: "var(--bn-primary, #6366f1)",
          foreground: "var(--bn-primary-foreground, #ffffff)",
          hover: "var(--bn-primary-hover, #4f46e5)",
        },
        success: {
          ...colors.success,
          DEFAULT: "var(--bn-success, #10b981)",
        },
        danger: {
          ...colors.danger,
          DEFAULT: "var(--bn-danger, #ef4444)",
        },
        warning: {
          ...colors.warning,
          DEFAULT: "var(--bn-warning, #f59e0b)",
        },
        gray: colors.gray,
        "content-bg": "var(--bn-bg, #f8fafc)",
        "content-box": "var(--bn-surface, #ffffff)",
        sidebar: "var(--bn-sidebar, #ffffff)",
        foreground: "var(--bn-foreground, #1e293b)",
        "icon-muted": "var(--bn-muted, #94a3b8)",
        "bn-border": "var(--bn-border, #e2e8f0)",
      },
      borderRadius: {
        ...borderRadius,
        bn: "var(--bn-radius, 12px)",
      },
      fontFamily: typography.fontFamily,
      fontSize: typography.fontSize,
      boxShadow: {
        ...shadows,
        bn: "var(--bn-shadow, 0 4px 12px rgba(2, 6, 23, 0.08))",
      },
      backgroundImage: {
        "primary-gradient": "var(--bn-primary-gradient, linear-gradient(135deg, #3b82f6, #8b5cf6))",
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
