// @ts-check
import { defineConfig } from "astro/config";
import svelte from "@astrojs/svelte";
import tailwind from "@astrojs/tailwind";

// https://astro.build/config
export default defineConfig({
  output: "static",
  integrations: [svelte(), tailwind({ applyBaseStyles: false })],
  server: {
    port: 4321,
    strictPort: true,
    host: "127.0.0.1",
  },
  vite: {
    clearScreen: false,
    envPrefix: ["VITE_", "TAURI_"],
    resolve: {
      alias: {
        $lib: "/src/lib",
        $components: "/src/components",
      },
    },
    server: {
      strictPort: true,
    },
  },
});
