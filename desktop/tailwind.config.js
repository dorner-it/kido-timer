/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        display: ["Bricolage Grotesque", "ui-sans-serif", "system-ui", "sans-serif"],
        mono: [
          "JetBrains Mono",
          "ui-monospace",
          "SFMono-Regular",
          "Menlo",
          "Monaco",
          "Consolas",
          "monospace",
        ],
        stencil: ["Antonio", "Bricolage Grotesque", "sans-serif"],
      },
      colors: {
        ink: {
          900: "rgb(var(--c-ink-900) / <alpha-value>)",
          800: "rgb(var(--c-ink-800) / <alpha-value>)",
          700: "rgb(var(--c-ink-700) / <alpha-value>)",
          600: "rgb(var(--c-ink-600) / <alpha-value>)",
          500: "rgb(var(--c-ink-500) / <alpha-value>)",
          400: "rgb(var(--c-ink-400) / <alpha-value>)",
          300: "rgb(var(--c-ink-300) / <alpha-value>)",
          200: "rgb(var(--c-ink-200) / <alpha-value>)",
          100: "rgb(var(--c-ink-100) / <alpha-value>)",
          50: "rgb(var(--c-ink-50) / <alpha-value>)",
        },
        signal: {
          DEFAULT: "rgb(var(--c-signal) / <alpha-value>)",
          glow: "rgb(var(--c-signal-glow) / <alpha-value>)",
        },
        status: {
          inactive: "rgb(var(--c-status-inactive) / <alpha-value>)",
          armed: "rgb(var(--c-status-armed) / <alpha-value>)",
          running: "rgb(var(--c-status-running) / <alpha-value>)",
          captured: "rgb(var(--c-status-captured) / <alpha-value>)",
          confirmed: "rgb(var(--c-status-confirmed) / <alpha-value>)",
          unknown: "rgb(var(--c-status-unknown) / <alpha-value>)",
        },
      },
      boxShadow: {
        glow: "var(--shadow-glow)",
        "glow-running": "var(--shadow-glow-running)",
        "glow-confirmed": "var(--shadow-glow-confirmed)",
        "glow-captured": "var(--shadow-glow-captured)",
      },
      keyframes: {
        scan: {
          "0%": { backgroundPosition: "0% 0%" },
          "100%": { backgroundPosition: "0% 200%" },
        },
        pulseRing: {
          "0%, 100%": { opacity: "0.55" },
          "50%": { opacity: "1" },
        },
        flicker: {
          "0%, 100%": { opacity: "1" },
          "50%": { opacity: "0.85" },
        },
      },
      animation: {
        scan: "scan 9s linear infinite",
        "pulse-ring": "pulseRing 1.6s ease-in-out infinite",
        flicker: "flicker 2.4s ease-in-out infinite",
      },
    },
  },
  plugins: [],
};
