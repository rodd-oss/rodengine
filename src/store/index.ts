import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export const useAppStore = defineStore('app', () => {
  // Schema state
  const currentSchema = ref<any>(null)
  const selectedTable = ref<string>('')
  const tables = ref<any[]>([])
  
  // UI state
  const darkMode = ref(false)
  const sidebarCollapsed = ref(false)
  
  // Database connection state
  const databaseInitialized = ref(false)
  const databasePath = ref('')
  
  // Replication state
  const connectedClients = ref<any[]>([])
  const deltaStream = ref<any[]>([])
  
  // Actions
  const setCurrentSchema = (schema: any) => {
    currentSchema.value = schema
    tables.value = schema?.tables || []
  }
  
  const selectTable = (tableName: string) => {
    selectedTable.value = tableName
  }
  
  const toggleDarkMode = () => {
    darkMode.value = !darkMode.value
    if (darkMode.value) {
      document.documentElement.classList.add('dark')
    } else {
      document.documentElement.classList.remove('dark')
    }
  }
  
  const toggleSidebar = () => {
    sidebarCollapsed.value = !sidebarCollapsed.value
  }
  
  const setDatabaseInitialized = (initialized: boolean, path: string = '') => {
    databaseInitialized.value = initialized
    databasePath.value = path
  }
  
  const addConnectedClient = (client: any) => {
    connectedClients.value.push(client)
  }
  
  const addDelta = (delta: any) => {
    deltaStream.value.unshift(delta)
    if (deltaStream.value.length > 100) {
      deltaStream.value.pop()
    }
  }
  
  // Getters
  const selectedTableSchema = computed(() => {
    return tables.value.find(t => t.name === selectedTable.value)
  })
  
  const totalEntities = computed(() => {
    // TODO: fetch from database
    return 0
  })
  
  return {
    // State
    currentSchema,
    selectedTable,
    tables,
    darkMode,
    sidebarCollapsed,
    databaseInitialized,
    databasePath,
    connectedClients,
    deltaStream,
    
    // Actions
    setCurrentSchema,
    selectTable,
    toggleDarkMode,
    toggleSidebar,
    setDatabaseInitialized,
    addConnectedClient,
    addDelta,
    
    // Getters
    selectedTableSchema,
    totalEntities
  }
})