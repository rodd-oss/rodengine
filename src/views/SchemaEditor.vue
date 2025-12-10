<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { useAppStore } from '../store'
import { open, save } from '@tauri-apps/api/dialog'
import { readTextFile, writeTextFile } from '@tauri-apps/api/fs'

const appStore = useAppStore()
const schemaInput = ref('')
const activeTab = ref('tables')

onMounted(async () => {
  if (appStore.databaseInitialized && appStore.tables.length === 0) {
    try {
      await appStore.loadSchema()
    } catch (error) {
      console.error('Failed to load schema:', error)
    }
  }
})

watch(() => appStore.databaseInitialized, async (initialized) => {
  if (initialized && appStore.tables.length === 0) {
    await appStore.loadSchema()
  }
})

const validateSchema = () => {
  // TODO: implement schema validation
  console.log('Validate schema')
}

const loadSchemaFromFile = async () => {
  try {
    const selected = await open({
      filters: [{ name: 'TOML', extensions: ['toml'] }],
      title: 'Select Schema File'
    })
    if (!selected || Array.isArray(selected)) return
    const content = await readTextFile(selected)
    schemaInput.value = content
  } catch (error) {
    console.error('Failed to load schema file:', error)
  }
}

const saveSchemaToFile = async () => {
  try {
    const filePath = await save({
      filters: [{ name: 'TOML', extensions: ['toml'] }],
      title: 'Save Schema File'
    })
    if (!filePath) return
    await writeTextFile(filePath, schemaInput.value)
    console.log('Saved schema file:', filePath)
  } catch (error) {
    console.error('Failed to save schema file:', error)
  }
}

const applySchema = async () => {
  // TODO: send schema to backend to apply changes
  console.log('Apply schema changes')
}
</script>

<template>
  <div class="schema-editor">
    <div class="editor-header">
      <h2>Schema Editor</h2>
      <div class="tabs">
        <button 
          v-for="tab in ['tables', 'enums', 'custom_types']" 
          :key="tab"
          class="tab"
          :class="{ 'active': activeTab === tab }"
          @click="activeTab = tab"
        >
          {{ tab.replace('_', ' ').toUpperCase() }}
        </button>
      </div>
    </div>
    
    <div class="editor-content">
      <div class="schema-tree">
        <h3>Schema Tree</h3>
        <div class="tree">
          <div v-for="table in appStore.tables" :key="table.name" class="tree-item">
            <div class="tree-node" @click="appStore.selectTable(table.name)">
              📊 {{ table.name }}
            </div>
            <div v-if="appStore.selectedTable === table.name" class="tree-children">
              <div v-for="field in table.fields" :key="field.name" class="tree-child">
                • {{ field.name }}: {{ field.field_type }}
              </div>
            </div>
          </div>
        </div>
      </div>
      
      <div class="schema-details">
        <div v-if="appStore.selectedTable">
          <h3>Table: {{ appStore.selectedTable }}</h3>
          <div class="table-editor">
            <div class="field-list">
              <div v-for="field in appStore.selectedTableSchema?.fields" :key="field.name" class="field-item">
                <input v-model="field.name" placeholder="Field name" />
                <select v-model="field.field_type">
                  <option value="u32">u32</option>
                  <option value="u64">u64</option>
                  <option value="f32">f32</option>
                  <option value="f64">f64</option>
                  <option value="string">String</option>
                  <option value="bool">bool</option>
                </select>
                <input type="checkbox" v-model="field.nullable" /> Nullable
                <input type="checkbox" v-model="field.indexed" /> Indexed
                <button class="btn-delete">✕</button>
              </div>
            </div>
            <button class="btn-add-field">+ Add Field</button>
          </div>
        </div>
        <div v-else>
          <h3>No table selected</h3>
          <p>Select a table from the tree to edit its fields.</p>
        </div>
      </div>
      
      <div class="schema-raw">
        <h3>Raw TOML</h3>
        <textarea v-model="schemaInput" placeholder="Paste TOML schema here..." class="toml-input"></textarea>
        <div class="raw-actions">
          <button class="btn" @click="validateSchema">Validate</button>
          <button class="btn" @click="loadSchemaFromFile">Load Schema</button>
          <button class="btn" @click="saveSchemaToFile">Save Schema</button>
          <button class="btn btn-primary" @click="applySchema">Apply Schema</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.schema-editor {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.editor-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.tabs {
  display: flex;
  gap: 0.5rem;
}

.tab {
  padding: 0.5rem 1rem;
  border: 1px solid var(--border-color);
  background: none;
  border-radius: 4px;
  cursor: pointer;
  color: var(--text-secondary);
  transition: all 0.2s ease;
}

.tab.active {
  background-color: #396cd8;
  color: white;
  border-color: #396cd8;
}

.editor-content {
  flex: 1;
  display: grid;
  grid-template-columns: 1fr 2fr 1fr;
  gap: 1rem;
  height: calc(100vh - 200px);
}

.schema-tree, .schema-details, .schema-raw {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 1rem;
  background-color: var(--bg-secondary);
  overflow-y: auto;
}

.schema-tree h3, .schema-details h3, .schema-raw h3 {
  margin-top: 0;
  margin-bottom: 1rem;
  color: var(--text-primary);
}

.tree {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.tree-node {
  padding: 0.5rem;
  border-radius: 4px;
  cursor: pointer;
  transition: background-color 0.2s ease;
}

.tree-node:hover {
  background-color: var(--hover-color);
}

.tree-children {
  margin-left: 1rem;
  margin-top: 0.25rem;
  border-left: 2px solid var(--border-color);
  padding-left: 1rem;
}

.tree-child {
  padding: 0.25rem 0.5rem;
  font-size: 0.875rem;
  color: var(--text-secondary);
}

.field-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin-bottom: 1rem;
}

.field-item {
  display: flex;
  gap: 0.5rem;
  align-items: center;
  padding: 0.5rem;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background-color: var(--bg-primary);
}

.field-item input[type="text"], .field-item select {
  flex: 1;
  padding: 0.25rem 0.5rem;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background-color: var(--bg-primary);
  color: var(--text-primary);
}

.btn-delete {
  background: none;
  border: none;
  color: #ef4444;
  cursor: pointer;
  padding: 0.25rem;
}

.btn-add-field {
  width: 100%;
  padding: 0.5rem;
  border: 1px dashed var(--border-color);
  background: none;
  border-radius: 4px;
  cursor: pointer;
  color: var(--text-secondary);
}

.btn-add-field:hover {
  background-color: var(--hover-color);
}

.toml-input {
  width: 100%;
  height: 300px;
  font-family: monospace;
  padding: 0.75rem;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background-color: var(--bg-primary);
  color: var(--text-primary);
  resize: vertical;
}

.raw-actions {
  display: flex;
  gap: 0.5rem;
  margin-top: 1rem;
}

.raw-actions .btn {
  flex: 1;
  padding: 0.5rem;
}

.raw-actions .btn-primary {
  background-color: #396cd8;
  color: white;
  border-color: #396cd8;
}

.raw-actions .btn-primary:hover {
  background-color: #2c5bc7;
}
</style>