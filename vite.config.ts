import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and keeps its own console output.
//
// The watcher must not descend into `src-tauri/target`: cargo runs executables
// from that tree (build scripts, then the app it launches), and Windows locks a
// running image against being opened by a watcher. Node reports
// `EBUSY: resource busy or locked` for that handle, Vite's FSWatcher emits it as
// an unhandled error, and the dev server dies — which `tauri dev` reports as
// "beforeDevCommand terminated with a non-zero status code". Nothing there is a
// frontend source, so excluding it is both the fix and the cheaper dev server.
const ignored = [
  "**/src-tauri/target/**",
  "**/src-tauri/gen/**",
  "**/dist/**",
  "**/results/**",
  "**/logs/**",
  "**/screenshots/**",
  "**/qa-screenshots/**",
  "**/shots/**",
  "**/.git/**",
  "**/node_modules/**",
];

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    watch: {
      ignored,
      // The default depth is unlimited; capping it keeps the handle count flat
      // on a checkout this size.
      followSymlinks: false,
    },
  },
  build: {
    target: "chrome105",
    minify: "esbuild",
    sourcemap: false,
  },
});
