<script setup lang="ts">
import { useHomeShell } from '@/core/home/useHomeShell'
import HomeChrome from './HomeChrome.vue'

/**
 * Home page host.
 *
 * The page is rendered by a theme package; everything the Core keeps for itself is
 * the chrome in `HomeChrome.vue`. This file's only job is to build the view model
 * (`useHomeShell`) and hand it over, which is why it is deliberately tiny — all of
 * the data, capability and shell logic lives in `core/home/use*`.
 */
const { theme, home, dialogs, commands } = useHomeShell()
</script>

<template>
  <HomeChrome :theme="theme" :home="home" :dialogs="dialogs" :commands="commands" />
</template>
