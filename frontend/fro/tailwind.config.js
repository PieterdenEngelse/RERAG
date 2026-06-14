/** @type {import('tailwindcss').Config} */
// Guardrails: keep content scope tight to avoid scanning large directories and causing Node OOM.
// If you add new template locations, add them explicitly here.
module.exports = {
  content: [
    "./src/**/*.{rs,html,js,jsx,ts,tsx}",
    "./public/index.html",
    "./assets/**/*.html"
  ],
  darkMode: 'class',
  theme: {
    extend: {},
  },
  plugins: [
    require('daisyui'),
  ],
  daisyui: {
    themes: [
      {
        dark: {
          ...require("daisyui/src/theming/themes")["dark"],
          primary: "#0D98BA",
          "primary-content": "#ffffff",
        },
      },
    ],
    darkTheme: "dark",
    base: true,
    styled: true,
    utils: true,
    logs: false,
  },
};
