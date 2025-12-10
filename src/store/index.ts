import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

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
  
  // Actions that call Tauri commands
  const initDatabase = async (schemaPath: string) => {
    try {
      const version = await invoke<number>('init_database', { schemaPath })
      setDatabaseInitialized(true, schemaPath)
      // Load schema after initialization
      await loadSchema()
      return version
    } catch (error) {
      console.error('Failed to initialize database:', error)
      throw error
    }
  }

  const loadSchema = async () => {
    try {
      const schema = await invoke<any>('get_schema')
      setCurrentSchema(schema)
    } catch (error) {
      console.error('Failed to load schema:', error)
      throw error
    }
  }

  const loadTables = async () => {
    try {
      const tablesList = await invoke<string[]>('get_tables')
      tables.value = tablesList.map(name => ({ name, fields: [] }))
    } catch (error) {
      console.error('Failed to load tables:', error)
      throw error
    }
  }

  const startReplication = async () => {
    try {
      await invoke('start_replication')
    } catch (error) {
      console.error('Failed to start replication:', error)
      throw error
    }
  }

  const stopReplication = async () => {
    try {
      await invoke('stop_replication')
    } catch (error) {
      console.error('Failed to stop replication:', error)
      throw error
    }
  }

  const fetchConnectedClients = async () => {
    try {
      const count = await invoke<number>('get_connected_clients')
      // Update store? We have connectedClients array, but count only.
      // We'll just return count for now.
      return count
    } catch (error) {
      console.error('Failed to fetch connected clients:', error)
      throw error
    }
  }

  const createEntity = async () => {
    try {
      const entityId = await invoke<number>('create_entity')
      return entityId
    } catch (error) {
      console.error('Failed to create entity:', error)
      throw error
    }
  }

  const getEntityCount = async (tableName: string) => {
    try {
      const count = await invoke<number>('get_entity_count', { tableName })
      return count
    } catch (error) {
      console.error('Failed to get entity count:', error)
      throw error
    }
  }

  const fetchEntities = async (tableName: string, limit: number, offset: number) => {
    try {
      const entities = await invoke<[number, number[]][]>('fetch_entities', { tableName, limit, offset })
      return entities
    } catch (error) {
      console.error('Failed to fetch entities:', error)
      throw error
    }
  }

  const fetchEntitiesJson = async (tableName: string, limit: number, offset: number) => {
    try {
      const entities = await invoke<[number, any][]>('fetch_entities_json', { tableName, limit, offset })
      return entities
    } catch (error) {
      console.error('Failed to fetch entities as JSON:', error)
      throw error
    }
    }

  const insertComponent = async (tableName: string, entityId: number, data: any) => {
    try {
      await invoke('insert_component', { tableName, entityId, json: data })
    } catch (error) {
      console.error('Failed to insert component:', error)
      throw error
    }
  }

  const updateComponent = async (tableName: string, entityId: number, data: any) => {
    try {
      await invoke('update_component', { tableName, entityId, json: data })
    } catch (error) {
      console.error('Failed to update component:', error)
      throw error
    }
  }

  const deleteComponent = async (tableName: string, entityId: number) => {
    try {
      await invoke('delete_component', { tableName, entityId })
    } catch (error) {
      console.error('Failed to delete component:', error)
      throw error
    }
  }

  const commitDatabase = async () => {
    try {
      const version = await invoke<number>('commit_database')
      return version
    } catch (error) {
      console.error('Failed to commit database:', error)
      throw error
    }
  }
  
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
    initDatabase,
    loadSchema,
    loadTables,
    startReplication,
    stopReplication,
    fetchConnectedClients,
    createEntity,
    getEntityCount,
    fetchEntities,
    fetchEntitiesJson,
    insertComponent,
    updateComponent,
    deleteComponent,
    commitDatabase,
    
    // Getters
    selectedTableSchema,
    totalEntities
  }
})