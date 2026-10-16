<script setup lang="ts">
import { onUnmounted } from 'vue'
import { useHomeShell } from '@/core/home/useHomeShell'
import { homeShell } from '@/core/home/homeShellContext'
import { useTheme } from '@/hooks/useTheme'
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
 * The view model is registered so a surface route can mount into the same runtime.
 */

// Resolving the installed theme package — and the wallpaper scheme, Naive
// overrides and runtime grant that follow from it — belongs to the shell,
// because the shell is the only thing that renders a theme. It is called here
// rather than in `App.vue` so the package resolver and its API stay off the
// Core-only routes, where no theme exists to resolve.
useTheme()

const shell = useHomeShell()
const { theme, home, dialogs, commands } = shell
homeShell.value = shell
onUnmounted(() => { homeShell.value = null })
</script>

<template>
  <HomeChrome :theme="theme" :home="home" :dialogs="dialogs" :commands="commands" />
</template>
