<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useAppStore } from '../store'

const appStore = useAppStore()
const searchQuery = ref('')
const currentPage = ref(1)
const itemsPerPage = ref(20)
const entities = ref<any[]>([])
const totalEntities = ref(0)
  const loading = ref(false)
  const error = ref('')
  const showEditModal = ref(false)
  const editingEntityId = ref<number | null>(null)
  const editJsonString = ref('')
  const editError = ref('')

// Load entities when table selected
watch(() => appStore.selectedTable, async (tableName) => {
  if (tableName) {
    await refreshData()
  }
})

onMounted(() => {
  if (appStore.selectedTable) {
    refreshData()
  }
})

  const refreshData = async () => {
    if (!appStore.selectedTable) return
    loading.value = true
    error.value = ''
    try {
      // Fetch total count
      totalEntities.value = await appStore.getEntityCount(appStore.selectedTable)
      // Fetch paginated data as JSON
      const limit = itemsPerPage.value
      const offset = (currentPage.value - 1) * limit
      const data = await appStore.fetchEntitiesJson(appStore.selectedTable, limit, offset)
      // Transform data: each entry is [entityId, componentData]
      entities.value = data.map(([entityId, componentData]) => ({
        id: entityId,
        ...componentData
      }))
      console.log('Fetched', data.length, 'entities for table:', appStore.selectedTable)
    } catch (err) {
      console.error('Failed to fetch entities:', err)
      error.value = err instanceof Error ? err.message : String(err)
      entities.value = []
    } finally {
      loading.value = false
    }
  }

  const insertRow = async () => {
    error.value = ''
    try {
      const tableName = appStore.selectedTable
      if (!tableName) {
        error.value = 'No table selected'
        return
      }
      const entityId = await appStore.createEntity()
      console.log('Created entity with ID:', entityId)
      // Insert component with default values
      await appStore.insertComponent(tableName, entityId, defaultRowData.value)
      console.log('Inserted component with default data')
      await refreshData()
    } catch (err) {
      console.error('Failed to insert row:', err)
      error.value = err instanceof Error ? err.message : String(err)
    }
  }

 const exportCSV = () => {
   // TODO: implement CSV export
   console.log('Export CSV')
 }

  const deleteRow = async (entityId: number) => {
    error.value = ''
    try {
      const tableName = appStore.selectedTable
      if (!tableName) {
        error.value = 'No table selected'
        return
      }
      await appStore.deleteComponent(tableName, entityId)
      console.log('Deleted component for entity:', entityId)
      await refreshData()
    } catch (err) {
      console.error('Failed to delete row:', err)
      error.value = err instanceof Error ? err.message : String(err)
    }
  }

 const editRow = (row: any) => {
   console.log('Edit row:', row)
   // Extract component data (excluding id)
   const { id, ...componentData } = row
   editingEntityId.value = id
   editJsonString.value = JSON.stringify(componentData, null, 2)
   editError.value = ''
   showEditModal.value = true
 }

 const saveEdit = async () => {
   try {
     const tableName = appStore.selectedTable
     if (!tableName || !editingEntityId.value) {
       return
     }
     const data = JSON.parse(editJsonString.value)
     await appStore.updateComponent(tableName, editingEntityId.value, data)
     console.log('Updated component for entity:', editingEntityId.value)
     showEditModal.value = false
     await refreshData()
   } catch (error) {
     editError.value = error instanceof Error ? error.message : String(error)
     console.error('Failed to save edit:', error)
   }
 }

  const commitDatabase = async () => {
    error.value = ''
    try {
      const version = await appStore.commitDatabase()
      console.log('Database committed, version:', version)
      await refreshData()
    } catch (err) {
      console.error('Failed to commit database:', err)
      error.value = err instanceof Error ? err.message : String(err)
    }
  }

const filteredData = computed(() => {
  if (!searchQuery.value) return entities.value
  const query = searchQuery.value.toLowerCase()
  return entities.value.filter(item => {
    // Search across all top-level fields
    for (const key in item) {
      const value = item[key]
      if (typeof value === 'string' && value.toLowerCase().includes(query)) {
        return true
      }
      if (typeof value === 'number' && value.toString().includes(query)) {
        return true
      }
    }
    return false
  })
})

const paginatedData = computed(() => {
  const start = (currentPage.value - 1) * itemsPerPage.value
  return filteredData.value.slice(start, start + itemsPerPage.value)
})

const totalPages = computed(() => 
  Math.ceil(filteredData.value.length / itemsPerPage.value)
)

 const columns = computed(() => {
   const schema = appStore.selectedTableSchema
   if (!schema || !schema.fields || schema.fields.length === 0) {
     // Fallback to static columns if no schema
     return [
       { key: 'id', label: 'ID', width: '80px' },
       { key: 'name', label: 'Name', width: '200px' },
       { key: 'position', label: 'Position', width: '200px' },
       { key: 'health', label: 'Health', width: '100px' },
       { key: 'timestamp', label: 'Timestamp', width: '200px' }
     ]
   }
   // Build columns from schema fields
   const fieldColumns = schema.fields.map((field: any) => ({
     key: field.name,
     label: field.name,
     width: field.name.length * 10 + 80 + 'px' // rough heuristic
   }))
   // Prepend entity ID column
   return [
     { key: 'id', label: 'ID', width: '80px' },
     ...fieldColumns
   ]
 })

 const defaultRowData = computed(() => {
   const schema = appStore.selectedTableSchema
   if (!schema || !schema.fields || schema.fields.length === 0) {
     return {}
   }
   const data: Record<string, any> = {}
   for (const field of schema.fields) {
     // Determine default value based on field type
     const fieldType = field.type?.toLowerCase() || ''
     if (fieldType.includes('u8') || fieldType.includes('u16') || fieldType.includes('u32') || fieldType.includes('u64') ||
         fieldType.includes('i8') || fieldType.includes('i16') || fieldType.includes('i32') || fieldType.includes('i64')) {
       data[field.name] = 0
     } else if (fieldType.includes('f32') || fieldType.includes('f64')) {
       data[field.name] = 0.0
     } else if (fieldType.includes('bool')) {
       data[field.name] = false
     } else if (fieldType.includes('string')) {
       data[field.name] = ''
     } else if (fieldType.includes('array')) {
       // Default empty array
       data[field.name] = []
     } else if (fieldType.includes('struct') || fieldType.includes('custom') || fieldType.includes('enum')) {
       // For nested types, default to empty object
       data[field.name] = {}
     } else {
       // Fallback to null
       data[field.name] = null
     }
   }
   return data
 })
</script>

<template>
  <div class="data-viewer">
    <div class="viewer-header">
      <h2>Data Viewer</h2>
      <div class="controls">
        <div class="table-selector">
          <label for="table-select">Table:</label>
          <select id="table-select" v-model="appStore.selectedTable">
            <option value="">Select a table</option>
            <option v-for="table in appStore.tables" :key="table.name" :value="table.name">
              {{ table.name }}
            </option>
          </select>
        </div>
        
        <div class="search">
          <input 
            v-model="searchQuery" 
            placeholder="Search entities..." 
            type="search"
          />
        </div>
        
         <div class="actions">
           <button class="btn" @click="refreshData">Refresh</button>
           <button class="btn" @click="exportCSV">Export CSV</button>
           <button class="btn btn-primary" @click="insertRow">Insert Row</button>
           <button class="btn btn-secondary" @click="commitDatabase">Commit</button>
         </div>
      </div>
     </div>
     
     <div v-if="error" class="error-message">
       {{ error }}
     </div>
     
     <div v-if="loading" class="loading">
       Loading...
     </div>
     
     <div class="data-grid">
      <div class="grid-header">
        <div class="grid-row">
          <div 
            v-for="col in columns" 
            :key="col.key" 
            class="grid-cell header-cell"
            :style="{ width: col.width }"
          >
            {{ col.label }}
          </div>
          <div class="grid-cell header-cell" style="width: 100px">Actions</div>
        </div>
      </div>
      
      <div class="grid-body">
        <div v-for="row in paginatedData" :key="row.id" class="grid-row">
          <div 
            v-for="col in columns" 
            :key="col.key" 
            class="grid-cell"
            :style="{ width: col.width }"
          >
            {{ row[col.key] }}
          </div>
          <div class="grid-cell actions-cell" style="width: 100px">
             <button class="btn-icon" title="Edit" @click="editRow(row)">✏️</button>
             <button class="btn-icon" title="Delete" @click="deleteRow(row.id)">🗑️</button>
          </div>
        </div>
      </div>
    </div>
    
    <div class="pagination">
      <button 
        class="btn" 
        :disabled="currentPage === 1"
        @click="currentPage--"
      >
        Previous
      </button>
      
      <div class="page-info">
        Page {{ currentPage }} of {{ totalPages }}
        <span class="total-rows">({{ filteredData.length }} rows)</span>
      </div>
      
      <button 
        class="btn" 
        :disabled="currentPage === totalPages"
        @click="currentPage++"
      >
        Next
      </button>
      
      <select v-model="itemsPerPage" class="page-size">
        <option value="10">10 per page</option>
        <option value="20">20 per page</option>
        <option value="50">50 per page</option>
        <option value="100">100 per page</option>
      </select>
     </div>

    <!-- Edit Modal -->
    <div v-if="showEditModal" class="modal-overlay">
      <div class="modal-content">
        <h3>Edit Component Data</h3>
        <div class="modal-body">
          <textarea 
            v-model="editJsonString" 
            rows="15" 
            style="width: 100%; font-family: monospace; padding: 0.5rem;"
            spellcheck="false"
          ></textarea>
          <div v-if="editError" class="error-message">{{ editError }}</div>
        </div>
        <div class="modal-actions">
          <button class="btn" @click="showEditModal = false">Cancel</button>
          <button class="btn btn-primary" @click="saveEdit">Save</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.data-viewer {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.viewer-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 1rem;
}

.controls {
  display: flex;
  align-items: center;
  gap: 1rem;
  flex-wrap: wrap;
}

.table-selector, .search {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.table-selector label, .search label {
  font-weight: 500;
  color: var(--text-secondary);
}

.table-selector select, .search input {
  padding: 0.5rem;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background-color: var(--bg-primary);
  color: var(--text-primary);
  min-width: 200px;
}

 .actions {
   display: flex;
   gap: 0.5rem;
 }

 .error-message {
   padding: 1rem;
   border: 1px solid #ef4444;
   border-radius: 4px;
   background-color: rgba(239, 68, 68, 0.1);
   color: #ef4444;
   margin-bottom: 1rem;
 }

 .loading {
   padding: 1rem;
   text-align: center;
   color: var(--text-secondary);
   margin-bottom: 1rem;
 }

 .data-grid {
  flex: 1;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  overflow: hidden;
  background-color: var(--bg-primary);
}

.grid-header {
  background-color: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
}

.grid-row {
  display: flex;
  border-bottom: 1px solid var(--border-color);
}

.grid-row:last-child {
  border-bottom: none;
}

.grid-cell {
  padding: 0.75rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.header-cell {
  font-weight: 600;
  color: var(--text-primary);
  background-color: var(--bg-secondary);
}

.grid-body .grid-row:hover {
  background-color: var(--hover-color);
}

.health-bar {
  display: inline-block;
  height: 4px;
  background-color: #10b981;
  margin-right: 0.5rem;
  vertical-align: middle;
}

.actions-cell {
  display: flex;
  gap: 0.25rem;
}

.btn-icon {
  background: none;
  border: none;
  cursor: pointer;
  padding: 0.25rem;
  font-size: 1rem;
  opacity: 0.7;
  transition: opacity 0.2s ease;
}

.btn-icon:hover {
  opacity: 1;
}

.btn-primary {
  background-color: #396cd8;
  color: white;
  border-color: #396cd8;
}

.btn-primary:hover {
  background-color: #2c5bc7;
}

.pagination {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 1rem;
  padding: 1rem;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background-color: var(--bg-secondary);
}

.page-info {
  color: var(--text-primary);
  font-weight: 500;
}

.total-rows {
  color: var(--text-secondary);
  font-size: 0.875rem;
  margin-left: 0.5rem;
}

.page-size {
  padding: 0.25rem 0.5rem;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background-color: var(--bg-primary);
  color: var(--text-primary);
}
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-content {
  background-color: var(--bg-primary);
  border-radius: 8px;
  padding: 1.5rem;
  max-width: 800px;
  width: 90%;
  max-height: 90vh;
  display: flex;
  flex-direction: column;
}

.modal-body {
  flex: 1;
  overflow-y: auto;
  margin: 1rem 0;
}

.modal-actions {
  display: flex;
  gap: 0.5rem;
  justify-content: flex-end;
}

.error-message {
  color: #ef4444;
  margin-top: 0.5rem;
  font-size: 0.875rem;
}
</style>