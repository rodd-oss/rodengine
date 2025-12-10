<script setup lang="ts">
import { useAppStore } from '../store'
import { RouterLink } from 'vue-router'

const appStore = useAppStore()

const menuItems = [
  { path: '/schema', icon: '📊', label: 'Schema Editor' },
  { path: '/data', icon: '📋', label: 'Data Viewer' },
  { path: '/query', icon: '🔍', label: 'Query Builder' },
  { path: '/replication', icon: '🔄', label: 'Replication' },
  { path: '/performance', icon: '📈', label: 'Performance' },
]
</script>

<template>
  <aside class="sidebar" :class="{ 'collapsed': appStore.sidebarCollapsed }">
    <div class="sidebar-header">
      <h2 v-if="!appStore.sidebarCollapsed">ECSDb Dashboard</h2>
      <h2 v-else>⚡</h2>
      <button class="sidebar-toggle" @click="appStore.toggleSidebar">
        {{ appStore.sidebarCollapsed ? '»' : '«' }}
      </button>
    </div>
    
    <nav class="sidebar-nav">
      <RouterLink 
        v-for="item in menuItems" 
        :key="item.path" 
        :to="item.path"
        class="nav-item"
        active-class="active"
      >
        <span class="nav-icon">{{ item.icon }}</span>
        <span v-if="!appStore.sidebarCollapsed" class="nav-label">{{ item.label }}</span>
      </RouterLink>
    </nav>
    
    <div class="sidebar-footer">
      <div v-if="!appStore.sidebarCollapsed" class="database-info">
        <div class="database-status" :class="{ 'connected': appStore.databaseInitialized }">
          {{ appStore.databaseInitialized ? '● Connected' : '○ Disconnected' }}
        </div>
        <div v-if="appStore.databasePath" class="database-path">
          {{ appStore.databasePath }}
        </div>
      </div>
      <button class="theme-toggle" @click="appStore.toggleDarkMode">
        {{ appStore.darkMode ? '☀️' : '🌙' }}
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 250px;
  background-color: var(--bg-secondary);
  border-right: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  transition: width 0.3s ease;
  overflow: hidden;
}

.sidebar.collapsed {
  width: 60px;
}

.sidebar-header {
  padding: 1rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-color);
}

.sidebar-header h2 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--text-primary);
  white-space: nowrap;
}

.sidebar-toggle {
  background: none;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  padding: 0.25rem 0.5rem;
  cursor: pointer;
  color: var(--text-secondary);
  transition: all 0.2s ease;
}

.sidebar-toggle:hover {
  background-color: var(--hover-color);
}

.sidebar-nav {
  flex: 1;
  padding: 1rem 0;
}

.nav-item {
  display: flex;
  align-items: center;
  padding: 0.75rem 1rem;
  color: var(--text-secondary);
  text-decoration: none;
  transition: all 0.2s ease;
  white-space: nowrap;
}

.nav-item:hover {
  background-color: var(--hover-color);
  color: var(--text-primary);
}

.nav-item.active {
  background-color: var(--hover-color);
  color: var(--text-primary);
  border-right: 3px solid #396cd8;
}

.nav-icon {
  font-size: 1.25rem;
  margin-right: 0.75rem;
  flex-shrink: 0;
}

.nav-label {
  font-size: 0.875rem;
  font-weight: 500;
}

.sidebar-footer {
  padding: 1rem;
  border-top: 1px solid var(--border-color);
}

.database-info {
  margin-bottom: 1rem;
}

.database-status {
  font-size: 0.75rem;
  color: #666;
  margin-bottom: 0.25rem;
}

.database-status.connected {
  color: #10b981;
}

.database-path {
  font-size: 0.7rem;
  color: #888;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.theme-toggle {
  background: none;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  padding: 0.5rem;
  cursor: pointer;
  width: 100%;
  font-size: 1.25rem;
  transition: all 0.2s ease;
}

.theme-toggle:hover {
  background-color: var(--hover-color);
}
</style>