import type { Config } from "tailwindcss";

const config: Config = {
  content: [
    "../../packages/ui/src/**/*.{js,ts,jsx,tsx}",
    "../../apps/desktop/src/**/*.{js,ts,jsx,tsx}",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {},
  },
  plugins: [],
};

export default config;
