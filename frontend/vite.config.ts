import path from 'path'
import type { PluginOption } from 'vite'
import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import { VitePWA } from 'vite-plugin-pwa'
import { createSvgIconsPlugin } from 'vite-plugin-svg-icons'

function setupPlugins(env: ImportMetaEnv): PluginOption[] {
  return [
    vue(),
    env.VITE_GLOB_APP_PWA === 'true' && VitePWA({
      injectRegister: 'auto',
      registerType: 'autoUpdate',
      workbox: {
        // `index.html` stays precached and stays the navigation fallback: it is
        // what makes the panel open at all with no network. Removing it (or
        // forcing navigations to the network) was tried and breaks offline.
        //
        // The stale-chunk 404 after an upgrade was not caused by caching the
        // entry document, but by the new worker never taking over: with
        // `registerType: 'autoUpdate'` the replacement is only *downloaded*, so
        // the old worker kept serving the previous build's `index.html`, whose
        // content-hashed chunk names no longer existed. `skipWaiting` +
        // `clientsClaim` below make the handover immediate, which is the actual
        // fix, and it costs offline nothing.
        navigateFallback: '/index.html',
        // `/clear.html` has to reach the network. Its whole job is to get a browser
        // out of a broken worker or a poisoned cache, so a navigation answered by
        // the cached app shell — or precached at all — would defeat it. Being
        // denied here means it needs a connection to open, which is the correct
        // trade for the one page whose purpose is escaping local state.
        //
        // A static page reached by path is still precached by default, which is
        // why the deny entry is present rather than implied: without it the
        // worker would answer the navigation with `index.html` and the page would
        // never run.
        navigateFallbackDenylist: [/^\/api(?:\/|$)/, /^\/clear\.html$/],
        globPatterns: ['index.html', 'assets/js/index.*.js', 'assets/js/vue-vendor.*.js', 'assets/*.css'],
        skipWaiting: true,
        clientsClaim: true,
        runtimeCaching: [{
          // Hashed build assets: safe to serve from cache first, and the hash
          // makes a cache hit correct by construction.
          urlPattern: /\/assets\//,
          handler: 'CacheFirst',
          options: {
            cacheName: 'yin-panel-runtime-assets',
            cacheableResponse: { statuses: [0, 200] },
            expiration: { maxEntries: 200, maxAgeSeconds: 7 * 24 * 60 * 60 },
          },
        }, {
          // A theme-rendered home needs three runtime requests, and offline it
          // needs all three: the package manifest, the sandbox grant, and the
          // resource bytes. Any one of them missing means the sandbox cannot boot
          // and the page silently drops to the C3 list.
          //
          // The paths are an explicit allowlist rather than a prefix match: every
          // other API call (bookmarks, spaces, monitor snapshots) stays
          // network-only, so no user data is ever written to a disk cache.
          //
          // The manifest and grant are keyed by URL, so `CacheFirst` would pin a
          // stale authorization or an outdated package after an upgrade. They are
          // therefore `NetworkFirst`: offline still falls back to the cached copy,
          // and online they are always the current ones.
          //
          // `StaleWhileRevalidate` was tried here and is wrong for these two paths.
          // It answers from the cache first and refreshes afterwards, so the *first*
          // page load after a theme upgrade runs the previous revision — and a theme
          // upgrade is exactly when behaviour changes. The symptom is a fix that is
          // present on the server and still not in effect, with no error anywhere:
          // the old theme simply runs. These are two small JSON responses and they
          // decide which theme boots, so they must not be served stale.
          //
          // The resource bytes stay `CacheFirst` because their URL carries the
          // package revision, which makes a hit correct by construction.
          // The matcher is serialized into sw.js and evaluated in the worker, so
          // it has to be self-contained: a reference to a module-level constant
          // would throw a ReferenceError at runtime and silently drop the rule.
          urlPattern: ({ url }) => ['/api/theme/v2/current', '/api/theme/v2/grants/'].some(pattern => url.pathname.startsWith(pattern)),
          handler: 'NetworkFirst',
          options: {
            cacheName: 'yin-panel-theme-metadata',
            cacheableResponse: { statuses: [0, 200] },
            // Without a timeout `NetworkFirst` waits on the network indefinitely, so a
            // panel opened offline would hang before it ever reached the cached copy.
            // Three seconds is long enough for a LAN round trip and short enough that
            // the C3 fallback still appears promptly.
            networkTimeoutSeconds: 3,
            expiration: { maxEntries: 8, maxAgeSeconds: 7 * 24 * 60 * 60 },
          },
        }, {
          // Theme resources are fetched by URL at runtime and the sandbox origin
          // may not touch the network, so without this the theme silently fails
          // offline and the page falls back to the C3 list. The URL carries the
          // package revision (a content hash), so a cache hit is correct by
          // construction — the same property the build assets rely on. The Core
          // additionally verifies every resource's sha256 after loading it, so
          // this does not weaken the integrity guarantee.
          urlPattern: /\/api\/theme\/v2\/assets\//,
          handler: 'CacheFirst',
          options: {
            cacheName: 'yin-panel-theme-assets',
            cacheableResponse: { statuses: [0, 200] },
            // Revisions are immutable, so entries never need evicting; the limit
            // only stops an unbounded pile-up across many upgrades.
            expiration: { maxEntries: 24, maxAgeSeconds: 30 * 24 * 60 * 60 },
          },
        }],
      },
      includeAssets: [
        'assets/favicon.svg',
        'assets/apple-touch-icon.png',
        'assets/bg-forest.webp',
        'assets/search_engine_svg/*.{png,svg}',
      ],
      manifest: {
        name: 'Yin-Panel',
        short_name: 'Yin-Panel',
      },
    }),
    createSvgIconsPlugin({
      // 更新 SVG 图标目录路径，指向 public/assets/svg-icons
      iconDirs: [path.resolve(process.cwd(), 'public/assets/svg-icons')],
      symbolId: '[name]',
    }),
  ]
}

export default defineConfig((env) => {
  const viteEnv = loadEnv(env.mode, process.cwd()) as unknown as ImportMetaEnv

  // 定义公共资源目录
  const publicDir = path.resolve(process.cwd(), 'public')
  // 定义资源目录
  const assetsDir = 'assets'
  
  return {
    base: '/', // 确保所有环境下基础路径一致
    publicDir, // 设置公共资源目录
    resolve: {
      alias: {
        '@': path.resolve(process.cwd(), 'src'),
        // 添加资源路径别名，指向 public/assets 目录
        '/assets': path.resolve(process.cwd(), 'public/assets'),
      },
    },
    plugins: setupPlugins(viteEnv),
    // 资源处理配置
    css: {
      devSourcemap: true,
    },
    // 静态资源处理
    assetsInclude: ['**/*.svg', '**/*.png', '**/*.jpg', '**/*.jpeg', '**/*.gif', '**/*.webp'],
    server: {
      host: '0.0.0.0',
      port: 1002,
      open: false,
      proxy: {
        '/api': {
          target: viteEnv.VITE_APP_API_BASE_URL,
          changeOrigin: true, // 允许跨域
          rewrite: path => path.replace('/api/', '/api/'),
        },
        '/uploads': {
          target: viteEnv.VITE_APP_API_BASE_URL,
          changeOrigin: true, // 允许跨域
          rewrite: path => path.replace('/uploads/', '/uploads/'),
        },
      },
    },
    build: {
      outDir: path.resolve(__dirname, '../backend/web'),
      reportCompressedSize: false,
      sourcemap: false,
      // Set assets directory to match development environment
      assetsDir,
      // Copy public directory assets to output directory
      copyPublicDir: true,
      // Use esbuild for minification instead of Terser
      minify: 'esbuild',
      // Increase chunk size warning limit to reduce warnings
      // Naive UI is kept in a shared vendor chunk; its current minified size
      // is about 790 kB and is loaded once by the application.
      chunkSizeWarningLimit: 850,
      // Empty the output directory before building
      emptyOutDir: true,
      commonjsOptions: {
        ignoreTryCatch: false,
      },
      // Configure drop_console for esbuild
      esbuildOptions: {
        drop: ['console'],
      },
      // Configure rollup options to ensure consistent asset paths
      rollupOptions: {
        output: {
          // Ensure asset filenames have consistent paths
          assetFileNames: `${assetsDir}/[name].[hash].[ext]`,
          chunkFileNames: `${assetsDir}/js/[name].[hash].js`,
          entryFileNames: `${assetsDir}/js/[name].[hash].js`,
          // Improve chunking to reduce large bundle sizes
          manualChunks(id) {
            if (id.includes('/node_modules/vue/') || id.includes('/node_modules/vue-router/') || id.includes('/node_modules/pinia/'))
              return 'vue-vendor'
          },
        },
      },
    },
  }
})
