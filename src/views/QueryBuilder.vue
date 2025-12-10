<script setup lang="ts">
import { ref } from 'vue'

const selectedTables = ref<string[]>([])
const selectedFields = ref<string[]>([])
const whereConditions = ref([{ field: '', operator: '=', value: '' }])
const joinConditions = ref([{ left: '', right: '', type: 'INNER' }])
const queryResult = ref<any[]>([])
const queryError = ref('')

const tables = [
  { name: 'entities', fields: ['id', 'version'] },
  { name: 'transform', fields: ['entity_id', 'position_x', 'position_y', 'position_z'] },
  { name: 'health', fields: ['entity_id', 'hp', 'max_hp'] }
]

const operators = ['=', '!=', '>', '<', '>=', '<=', 'LIKE', 'IN', 'IS NULL']

const addCondition = () => {
  whereConditions.value.push({ field: '', operator: '=', value: '' })
}

const removeCondition = (index: number) => {
  whereConditions.value.splice(index, 1)
}

const addJoin = () => {
  joinConditions.value.push({ left: '', right: '', type: 'INNER' })
}

const removeJoin = (index: number) => {
  joinConditions.value.splice(index, 1)
}

const executeQuery = () => {
  // Mock execution
  queryError.value = ''
  queryResult.value = Array.from({ length: 5 }, (_, i) => ({
    id: i + 1,
    entity_id: i + 1,
    position_x: Math.random() * 100,
    health: Math.floor(Math.random() * 100)
  }))
}

const exportQuery = () => {
  console.log('Export query')
}
</script>

<template>
  <div class="query-builder">
    <div class="builder-header">
      <h2>Query Builder</h2>
      <div class="query-actions">
        <button class="btn btn-primary" @click="executeQuery">
          Execute Query
        </button>
        <button class="btn" @click="exportQuery">
          Export Results
        </button>
      </div>
    </div>
    
    <div class="builder-content">
      <div class="builder-left">
        <div class="tables-panel">
          <h3>Tables</h3>
          <div class="table-list">
            <div 
              v-for="table in tables" 
              :key="table.name"
              class="table-item"
              :class="{ 'selected': selectedTables.includes(table.name) }"
              @click="selectedTables.includes(table.name) 
                ? selectedTables = selectedTables.filter(t => t !== table.name)
                : selectedTables.push(table.name)"
            >
              📊 {{ table.name }}
              <div class="field-list">
                <div 
                  v-for="field in table.fields" 
                  :key="field"
                  class="field-item"
                  :class="{ 'selected': selectedFields.includes(`${table.name}.${field}`) }"
                  @click.stop="selectedFields.includes(`${table.name}.${field}`) 
                    ? selectedFields = selectedFields.filter(f => f !== `${table.name}.${field}`)
                    : selectedFields.push(`${table.name}.${field}`)"
                >
                  • {{ field }}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
      
      <div class="builder-center">
        <div class="query-panel">
          <h3>Query Conditions</h3>
          
          <div class="section">
            <h4>WHERE</h4>
            <div v-for="(cond, index) in whereConditions" :key="index" class="condition">
               <select v-model="cond.field" class="field-select">
                 <option value="">Select field</option>
                 <optgroup v-for="table in tables" :key="table.name" :label="table.name">
                   <option v-for="field in table.fields" :key="field" :value="`${table.name}.${field}`">
                     {{ table.name }}.{{ field }}
                   </option>
                 </optgroup>
               </select>
              
              <select v-model="cond.operator" class="operator-select">
                <option v-for="op in operators" :key="op" :value="op">{{ op }}</option>
              </select>
              
              <input v-model="cond.value" type="text" placeholder="Value" class="value-input" />
              
              <button class="btn-remove" @click="removeCondition(index)">✕</button>
            </div>
            <button class="btn-add" @click="addCondition">+ Add Condition</button>
          </div>
          
          <div class="section">
            <h4>JOIN</h4>
            <div v-for="(join, index) in joinConditions" :key="index" class="join">
              <select v-model="join.left" class="join-select">
                <option value="">Left table.field</option>
                <option v-for="table in tables" :key="table.name">
                  <optgroup :label="table.name">
                    <option v-for="field in table.fields" :key="field" :value="`${table.name}.${field}`">
                      {{ table.name }}.{{ field }}
                    </option>
                  </optgroup>
                </option>
              </select>
              
              <select v-model="join.type" class="join-type">
                <option value="INNER">INNER JOIN</option>
                <option value="LEFT">LEFT JOIN</option>
                <option value="RIGHT">RIGHT JOIN</option>
              </select>
              
              <select v-model="join.right" class="join-select">
                <option value="">Right table.field</option>
                <option v-for="table in tables" :key="table.name">
                  <optgroup :label="table.name">
                    <option v-for="field in table.fields" :key="field" :value="`${table.name}.${field}`">
                      {{ table.name }}.{{ field }}
                    </option>
                  </optgroup>
                </option>
              </select>
              
              <button class="btn-remove" @click="removeJoin(index)">✕</button>
            </div>
            <button class="btn-add" @click="addJoin">+ Add Join</button>
          </div>
        </div>
      </div>
      
      <div class="builder-right">
        <div class="results-panel">
          <h3>Results</h3>
          <div v-if="queryError" class="error">
            {{ queryError }}
          </div>
          <div v-else-if="queryResult.length === 0" class="no-results">
            No results yet. Execute a query to see results.
          </div>
          <div v-else class="results-grid">
            <div class="results-header">
              <div v-for="key in Object.keys(queryResult[0])" :key="key" class="results-cell">
                {{ key }}
              </div>
            </div>
            <div v-for="(row, rowIndex) in queryResult" :key="rowIndex" class="results-row">
              <div v-for="(value, valueIndex) in Object.values(row)" :key="valueIndex" class="results-cell">
                {{ value }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
    
    <div class="query-preview">
      <h3>Generated Query</h3>
      <pre class="preview-code">
SELECT {{ selectedFields.join(', ') || '*' }}
FROM {{ selectedTables.join(', ') || 'entities' }}
{{ joinConditions.map(j => `${j.type} JOIN ${j.right.split('.')[0]} ON ${j.left} = ${j.right}`).join('\n') }}
{{ whereConditions.length > 0 ? 'WHERE ' + whereConditions.map(c => `${c.field} ${c.operator} ${c.value}`).join(' AND ') : '' }}
      </pre>
    </div>
  </div>
</template>

<style scoped>
.query-builder {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.builder-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.query-actions {
  display: flex;
  gap: 0.5rem;
}

.builder-content {
  flex: 1;
  display: grid;
  grid-template-columns: 1fr 2fr 1fr;
  gap: 1rem;
  height: calc(100vh - 250px);
}

.builder-left, .builder-center, .builder-right {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 1rem;
  background-color: var(--bg-secondary);
  overflow-y: auto;
}

.tables-panel h3, .query-panel h3, .results-panel h3 {
  margin-top: 0;
  margin-bottom: 1rem;
  color: var(--text-primary);
}

.table-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.table-item {
  padding: 0.75rem;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s ease;
  background-color: var(--bg-primary);
}

.table-item.selected {
  border-color: #396cd8;
  background-color: rgba(57, 108, 216, 0.1);
}

.field-list {
  margin-top: 0.5rem;
  margin-left: 1rem;
}

.field-item {
  padding: 0.25rem 0.5rem;
  font-size: 0.875rem;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: 2px;
}

.field-item.selected {
  color: #396cd8;
  background-color: rgba(57, 108, 216, 0.1);
}

.section {
  margin-bottom: 1.5rem;
}

.section h4 {
  margin: 0 0 0.5rem 0;
  color: var(--text-secondary);
  font-size: 0.875rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.condition, .join {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 0.5rem;
  align-items: center;
}

.field-select, .operator-select, .value-input, .join-select, .join-type {
  padding: 0.5rem;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background-color: var(--bg-primary);
  color: var(--text-primary);
  flex: 1;
}

.btn-remove {
  background: none;
  border: none;
  color: #ef4444;
  cursor: pointer;
  padding: 0.25rem;
  flex-shrink: 0;
}

.btn-add {
  width: 100%;
  padding: 0.5rem;
  border: 1px dashed var(--border-color);
  background: none;
  border-radius: 4px;
  cursor: pointer;
  color: var(--text-secondary);
  margin-top: 0.5rem;
}

.btn-add:hover {
  background-color: var(--hover-color);
}

.results-panel {
  display: flex;
  flex-direction: column;
}

.error {
  color: #ef4444;
  padding: 1rem;
  border: 1px solid #ef4444;
  border-radius: 4px;
  background-color: rgba(239, 68, 68, 0.1);
}

.no-results {
  color: var(--text-secondary);
  text-align: center;
  padding: 2rem;
}

.results-grid {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.results-header {
  display: flex;
  border-bottom: 2px solid var(--border-color);
  font-weight: 600;
  color: var(--text-primary);
}

.results-row {
  display: flex;
  border-bottom: 1px solid var(--border-color);
}

.results-row:hover {
  background-color: var(--hover-color);
}

.results-cell {
  padding: 0.5rem;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 80px;
}

.query-preview {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 1rem;
  background-color: var(--bg-secondary);
}

.preview-code {
  margin: 0;
  padding: 1rem;
  background-color: var(--bg-primary);
  border-radius: 4px;
  font-family: monospace;
  font-size: 0.875rem;
  color: var(--text-primary);
  white-space: pre-wrap;
  overflow-x: auto;
}
</style>