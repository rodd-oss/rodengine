<script setup lang="ts">
import { ref, computed } from 'vue'
import { useAppStore } from '../store'

const appStore = useAppStore()
const searchQuery = ref('')
const currentPage = ref(1)
const itemsPerPage = ref(20)

// Mock data for demonstration
const mockData = Array.from({ length: 100 }, (_, i) => ({
  id: i + 1,
  name: `Entity ${i + 1}`,
  position: { x: Math.random() * 100, y: Math.random() * 100, z: Math.random() * 100 },
  health: Math.floor(Math.random() * 100),
  timestamp: new Date().toISOString()
}))

const filteredData = computed(() => {
  if (!searchQuery.value) return mockData
  return mockData.filter(item => 
    item.name.toLowerCase().includes(searchQuery.value.toLowerCase())
  )
})

const paginatedData = computed(() => {
  const start = (currentPage.value - 1) * itemsPerPage.value
  return filteredData.value.slice(start, start + itemsPerPage.value)
})

const totalPages = computed(() => 
  Math.ceil(filteredData.value.length / itemsPerPage.value)
)

const columns = [
  { key: 'id', label: 'ID', width: '80px' },
  { key: 'name', label: 'Name', width: '200px' },
  { key: 'position', label: 'Position', width: '200px' },
  { key: 'health', label: 'Health', width: '100px' },
  { key: 'timestamp', label: 'Timestamp', width: '200px' }
]
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
          <button class="btn">Refresh</button>
          <button class="btn">Export CSV</button>
          <button class="btn btn-primary">Insert Row</button>
        </div>
      </div>
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
          <div class="grid-cell" style="width: 80px">{{ row.id }}</div>
          <div class="grid-cell" style="width: 200px">{{ row.name }}</div>
          <div class="grid-cell" style="width: 200px">
            ({{ row.position.x.toFixed(2) }}, {{ row.position.y.toFixed(2) }}, {{ row.position.z.toFixed(2) }})
          </div>
          <div class="grid-cell" style="width: 100px">
            <span class="health-bar" :style="{ width: `${row.health}%` }"></span>
            {{ row.health }}
          </div>
          <div class="grid-cell" style="width: 200px">{{ row.timestamp }}</div>
          <div class="grid-cell actions-cell" style="width: 100px">
            <button class="btn-icon" title="Edit">✏️</button>
            <button class="btn-icon" title="Delete">🗑️</button>
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
</style>