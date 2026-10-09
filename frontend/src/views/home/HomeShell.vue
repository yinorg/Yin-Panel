<script setup lang="ts">
import { provide } from 'vue'
import { useHomeShell } from '@/core/home/useHomeShell'
import HomeChrome from './HomeChrome.vue'

/**
 * The application shell.
 *
 * It is rendered by `App.vue` for every themed route, outside the router view, so
 * the theme runtime and the home data live at application level rather than inside
 * one page. The Core renders no page of its own: this component owns the chrome,
 * the runtime host and the C3 fallback, and the matched route only mounts a view
 * into the runtime (`/theme/:view` → `ThemeSurface`, otherwise nothing).
 *
 * The view model is provided so a surface route can mount into the same runtime.
 */
const shell = useHomeShell()
const { theme, home, dialogs, commands } = shell
provide('yin-home-shell', shell)
</script>

<template>
  <HomeChrome :theme="theme" :home="home" :dialogs="dialogs" :commands="commands" />
</template>
