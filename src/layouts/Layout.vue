<script setup lang="ts">
import { useAppStore } from '../store'
import Sidebar from '../components/Sidebar.vue'
import Header from '../components/Header.vue'

const appStore = useAppStore()
</script>

<template>
  <div class="app-layout" :class="{ 'dark': appStore.darkMode, 'sidebar-collapsed': appStore.sidebarCollapsed }">
    <Sidebar />
    <div class="main-content">
      <Header />
      <main class="content">
        <router-view />
      </main>
    </div>
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  transition: background-color 0.3s ease;
}

.dark {
  --bg-primary: #1a1a1a;
  --bg-secondary: #2d2d2d;
  --text-primary: #f0f0f0;
  --text-secondary: #a0a0a0;
  --border-color: #404040;
  --hover-color: #3a3a3a;
}

:root {
  --bg-primary: #ffffff;
  --bg-secondary: #f5f5f5;
  --text-primary: #333333;
  --text-secondary: #666666;
  --border-color: #e0e0e0;
  --hover-color: #f0f0f0;
}

.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background-color: var(--bg-primary);
}

.content {
  flex: 1;
  padding: 1.5rem;
  overflow-y: auto;
}
</style>