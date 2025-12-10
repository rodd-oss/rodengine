<script setup lang="ts">
import { useAppStore } from '../store'

const appStore = useAppStore()

import { open, save } from '@tauri-apps/plugin-dialog'
import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs'

const openSchemaFile = async () => {
  try {
    const selected = await open({
      filters: [{ name: 'TOML', extensions: ['toml'] }],
      title: 'Select Schema File'
    })
    if (!selected || Array.isArray(selected)) return
    await readTextFile(selected) // Read schema content (currently unused)
    // TODO: pass content to schema editor, maybe via store
    console.log('Loaded schema file:', selected)
  } catch (error) {
    console.error('Failed to open schema file:', error)
  }
}

const saveSchemaFile = async () => {
  try {
    const filePath = await save({
      filters: [{ name: 'TOML', extensions: ['toml'] }],
      title: 'Save Schema File'
    })
    if (!filePath) return
    // TODO: get schema content from store or editor
    await writeTextFile(filePath, '')
    console.log('Saved schema file:', filePath)
  } catch (error) {
    console.error('Failed to save schema file:', error)
  }
}

const initDatabase = async () => {
  if (appStore.databaseInitialized) {
    console.log('Database already initialized')
    return
  }
  try {
    const selected = await open({
      filters: [{ name: 'TOML', extensions: ['toml'] }],
      title: 'Select Schema File to Initialize Database'
    })
    if (!selected || Array.isArray(selected)) return
    await appStore.initDatabase(selected)
  } catch (error) {
    console.error('Failed to initialize database:', error)
  }
}
</script>

<template>
  <header class="header">
    <div class="header-left">
      <h1 class="page-title">
        <slot name="title">
          {{ $route.name }}
        </slot>
      </h1>
      <div v-if="appStore.selectedTable" class="table-badge">
        {{ appStore.selectedTable }}
      </div>
    </div>
    
    <div class="header-right">
      <div class="actions">
        <button class="btn btn-secondary" @click="openSchemaFile">
          Import Schema
        </button>
        <button class="btn btn-secondary" @click="saveSchemaFile">
          Export Schema
        </button>
        <button 
          class="btn btn-primary" 
          :class="{ 'btn-success': appStore.databaseInitialized }"
          @click="initDatabase"
        >
          {{ appStore.databaseInitialized ? 'Database Connected' : 'Initialize Database' }}
        </button>
      </div>
      
      <div class="stats">
        <div class="stat">
          <span class="stat-label">Tables</span>
          <span class="stat-value">{{ appStore.tables.length }}</span>
        </div>
        <div class="stat">
          <span class="stat-label">Entities</span>
          <span class="stat-value">{{ appStore.totalEntities }}</span>
        </div>
        <div class="stat">
          <span class="stat-label">Clients</span>
          <span class="stat-value">{{ appStore.connectedClients.length }}</span>
        </div>
      </div>
    </div>
  </header>
</template>

<style scoped>
.header {
  padding: 1rem 1.5rem;
  border-bottom: 1px solid var(--border-color);
  background-color: var(--bg-primary);
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.page-title {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--text-primary);
}

.table-badge {
  background-color: #e3f2fd;
  color: #1976d2;
  padding: 0.25rem 0.75rem;
  border-radius: 1rem;
  font-size: 0.875rem;
  font-weight: 500;
}

.dark .table-badge {
  background-color: #0d47a1;
  color: #bbdefb;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 2rem;
}

.actions {
  display: flex;
  gap: 0.5rem;
}

.btn {
  padding: 0.5rem 1rem;
  border-radius: 6px;
  border: 1px solid var(--border-color);
  background-color: var(--bg-secondary);
  color: var(--text-primary);
  font-size: 0.875rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.btn:hover {
  background-color: var(--hover-color);
}

.btn-primary {
  background-color: #396cd8;
  color: white;
  border-color: #396cd8;
}

.btn-primary:hover {
  background-color: #2c5bc7;
}

.btn-success {
  background-color: #10b981;
  border-color: #10b981;
}

.btn-success:hover {
  background-color: #0ea271;
}

.stats {
  display: flex;
  gap: 1.5rem;
}

.stat {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.stat-label {
  font-size: 0.75rem;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.stat-value {
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--text-primary);
}
</style>