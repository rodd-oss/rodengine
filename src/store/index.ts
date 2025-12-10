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
  const pendingDeltaCount = ref(0)
  const conflictLog = ref<any[]>([])
  
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

  const fetchClients = async () => {
    try {
      const clients = await invoke<any[]>('get_clients')
      console.log('Fetched clients raw:', clients)
      // Map to UI shape
      connectedClients.value = clients.map(client => {
        // Extract ID: ClientId serializes as a tuple array [uuid]
        let id: string
        if (Array.isArray(client.id)) {
          id = client.id[0]
        } else if (client.id && typeof client.id === 'object' && client.id[0] !== undefined) {
          id = client.id[0]
        } else {
          id = String(client.id)
        }
        return {
          id,
          address: client.addr,
          version: '1.0.0', // placeholder
          lastHeartbeat: new Date().toISOString(), // placeholder
          lag: 0, // placeholder
          status: client.state.toLowerCase()
        }
      })
    } catch (error) {
      console.error('Failed to fetch clients:', error)
      throw error
    }
  }

  const fetchPendingDeltaCount = async () => {
    try {
      const count = await invoke<number>('get_pending_delta_count')
      pendingDeltaCount.value = count
      return count
    } catch (error) {
      console.error('Failed to fetch pending delta count:', error)
      throw error
    }
  }

  const fetchConflictLog = async () => {
    try {
      const conflicts = await invoke<any[]>('get_conflict_log')
      conflictLog.value = conflicts.map(conflict => ({
        id: conflict.timestamp, // use timestamp as id
        type: conflict.field_offset !== null ? 'field' : 'row',
        table: conflict.table_name || conflict.table_id.toString(),
        entityId: conflict.entity_id,
        resolution: 'server-wins', // placeholder
        timestamp: new Date(conflict.timestamp).toISOString()
      }))
    } catch (error) {
      console.error('Failed to fetch conflict log:', error)
      throw error
    }
  }

  const fetchDeltaLog = async () => {
    try {
      const entries = await invoke<any[]>('get_delta_log')
      // Map to UI shape
      deltaStream.value = entries.map(entry => {
        // Map operation type to UI class
        let type = entry.first_op_type
        if (type === 'create_entity' || type === 'insert') type = 'insert'
        else if (type === 'delete_entity' || type === 'delete') type = 'delete'
        else if (type === 'update') type = 'update'
        else type = 'unknown'
        return {
          id: entry.seq,
          type,
          table: entry.first_table_id?.toString() || 'unknown',
          entityId: entry.first_entity_id || 0,
          timestamp: new Date(entry.timestamp).toISOString(),
          size: entry.operation_count * 10 // placeholder size
        }
      })
    } catch (error) {
      console.error('Failed to fetch delta log:', error)
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
    pendingDeltaCount,
    conflictLog,
    
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
    fetchClients,
    fetchPendingDeltaCount,
     fetchConflictLog,
     fetchDeltaLog,
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