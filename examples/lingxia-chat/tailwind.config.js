/** @type {import('tailwindcss').Config} */
export default {
  content: ['./pages/**/*.{ts,tsx,js,jsx,vue}', './lxapp.{ts,tsx}'],
  theme: {
    extend: {
      keyframes: {
        blink: {
          '0%, 49%': { opacity: '1' },
          '50%, 100%': { opacity: '0' },
        },
        'chart-in': {
          from: { opacity: '0', transform: 'scale(0.96) translateY(8px)' },
          to:   { opacity: '1', transform: 'scale(1)   translateY(0)' },
        },
      },
      animation: {
        blink:    'blink 0.9s step-end infinite',
        'chart-in': 'chart-in 0.35s cubic-bezier(0.16,1,0.3,1)',
      },
    },
  },
  plugins: [],
};
