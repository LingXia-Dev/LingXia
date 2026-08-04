import plugin from 'tailwindcss/plugin';
import {
  backgroundPalette,
  baseStyles,
  borderPalette,
  boxShadow,
  textPalette,
} from './theme/tokens.js';

/** @type {import('tailwindcss').Config} */
const config = {
  content: [
    "./pages/**/*.{ts,tsx,js,jsx,vue}",
    "./shared/**/*.{ts,tsx,js,jsx,vue}",
    "./lxapp.{ts,tsx,js,jsx}",
  ],
  // The runtime stamps the resolved appearance on <html>; platform media queries
  // can lag an in-place switch, so `dark:` keys off the attribute too.
  darkMode: ['selector', '[data-theme="dark"]'],
  theme: {
    extend: {
      // Every palette entry is a `rgb(var(--lx-…) / <alpha-value>)` reference, so
      // the existing utilities (`bg-white`, `text-gray-500`, `from-blue-50/50`)
      // follow the theme — including their alpha modifiers. See theme/tokens.js
      // for why background, text, and border resolve separately.
      colors: backgroundPalette,
      textColor: textPalette,
      borderColor: borderPalette,
      divideColor: borderPalette,
      placeholderColor: textPalette,
      boxShadow,
      fontFamily: {
        sans: [
          '-apple-system',
          'BlinkMacSystemFont',
          '"Segoe UI"',
          'Roboto',
          'Helvetica',
          'Arial',
          'sans-serif',
        ],
      },
      fontSize: {
        // iOS-style type scale
        'xs': ['12px', { lineHeight: '16px' }],
        'sm': ['14px', { lineHeight: '20px' }],
        'base': ['16px', { lineHeight: '24px' }],
        'lg': ['18px', { lineHeight: '28px' }],
        'xl': ['20px', { lineHeight: '28px' }],
        '2xl': ['24px', { lineHeight: '32px' }],
        '3xl': ['30px', { lineHeight: '36px' }],
        '4xl': ['36px', { lineHeight: '40px' }],
      },
      borderRadius: {
        // iOS-style border radius
        'sm': '4px',
        'DEFAULT': '8px',
        'md': '10px',
        'lg': '12px',
        'xl': '16px',
        '2xl': '20px',
        '3xl': '24px',
      },
      spacing: {
        // Safe area insets for mobile devices
        'safe': 'env(safe-area-inset-top)',
        'safe-b': 'env(safe-area-inset-bottom)',
        'safe-l': 'env(safe-area-inset-left)',
        'safe-r': 'env(safe-area-inset-right)',
      },
      backdropBlur: {
        'xs': '2px',
        'xl': '24px',
      },
      animation: {
        "fade-in": "fadeIn 0.5s ease-out",
        "slide-up": "slideUp 0.4s ease-out",
        "spin-slow": "spin 3s linear infinite",
        "pulse-slow": "pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite",
        "blink": "blink 0.9s step-end infinite",
        "chart-in": "chartIn 0.35s cubic-bezier(0.16,1,0.3,1)",
      },
      keyframes: {
        fadeIn: {
          "0%": { opacity: "0" },
          "100%": { opacity: "1" },
        },
        slideUp: {
          "0%": { opacity: "0", transform: "translateY(10px)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
        blink: {
          "0%, 49%": { opacity: "1" },
          "50%, 100%": { opacity: "0" },
        },
        chartIn: {
          from: { opacity: "0", transform: "scale(0.96) translateY(8px)" },
          to:   { opacity: "1", transform: "scale(1) translateY(0)" },
        },
      },
    },
  },
  plugins: [
    plugin(({ addBase }) => {
      addBase(baseStyles);
    }),
  ],
};

export default config;
