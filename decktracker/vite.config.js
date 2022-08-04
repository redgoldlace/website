import { defineConfig } from "vite";
import preact from "@preact/preset-vite";

// https://vitejs.dev/config/
export default defineConfig({
  base: "/decktracker/",
  plugins: [preact()],
  build: {
    rollupOptions: {
      output: {
        entryFileNames: "[name].js",
        assetFileNames: "assets/[name].[ext]"
      }
    }
  }
})
