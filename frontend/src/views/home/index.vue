<script setup lang="ts">
/**
 * A themed route renders no page of its own.
 *
 * `HomeShell` owns the theme runtime and the Core chrome, and the theme renders the
 * page. This component only exists so the router has a matched component for `/`,
 * `/:code` and the trusted route; `/theme/:view` has its own (`ThemeSurface.vue`).
 */
</script>

<template>
  <div hidden aria-hidden="true" />
</template>
