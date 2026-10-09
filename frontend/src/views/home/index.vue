<script setup lang="ts">
import { provide } from 'vue'
import { useHomeShell } from '@/core/home/useHomeShell'
import HomeChrome from './HomeChrome.vue'

/**
 * Home page host.
 *
 * The page is rendered by a theme package; everything the Core keeps for itself is
 * the chrome in `HomeChrome.vue`. This file's only job is to build the view model
 * (`useHomeShell`) and hand it over, which is why it is deliberately tiny — all of
 * the data, capability and shell logic lives in `core/home/use*`.
 *
 * The shell is also provided: a theme-contributed surface is rendered by the child
 * `theme/:view` route (`ThemeSurface.vue`), which mounts into the same runtime and
 * therefore needs the same view model.
 */
const shell = useHomeShell()
const { theme, home, dialogs, commands } = shell
provide('yin-home-shell', shell)
</script>

<template>
  <HomeChrome :theme="theme" :home="home" :dialogs="dialogs" :commands="commands" />
</template>
