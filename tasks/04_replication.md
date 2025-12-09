# Phase 4: Replication
**Estimated Time:** Weeks 13-16

## Overview
Enable multi‑client synchronization via delta‑based replication. Clients connect to the database server, receive initial full sync, then incremental updates. Support conflict resolution, network protocol, and client‑side library.

## Dependencies
- Phase 3 completed (persistence with delta tracking)
- Network stack (TCP or WebSocket) depending on deployment

## Subtasks

### 4.1 Client Connection Management
- **Server Listener**: Accept incoming client connections (TCP or WebSocket)
- **Client Session**: Per‑client state (ID, version, subscribed tables, network socket)
- **Authentication**: Optional simple auth (token‑based)
- **Connection Lifecycle**: Handle connect, disconnect, re‑connect with session resume

### 4.2 Delta Serialization Format
- **Binary Protocol**: Define frame header (magic, version, flags), metadata (DB version, timestamp), delta count, delta operations, checksum
- **DeltaEncoder**: Serialize `DeltaOp` to compact binary format
- **DeltaDecoder**: Deserialize binary to `DeltaOp` on client side
- **Compression**: Optional zstd compression for large delta batches

### 4.3 Network Broadcast Mechanism
- **Broadcast Queue**: Outbound delta batches for each client, with flow control
- **Broadcast Scheduler**: Send deltas to clients in background task(s)
- **Reliable Delivery**: ACK/retransmit mechanism (or rely on TCP)
- **Bandwidth Throttling**: Limit broadcast rate per client

### 4.4 Conflict Resolution
- **Conflict Detection**: Detect when client and server modify same field concurrently
- **Resolution Strategies**: Server‑authoritative, last‑write‑wins, custom merge functions
- **Conflict Log**: Record conflicts for debugging/analytics
- **Client‑Side Merge API**: Allow game logic to provide merge callbacks

### 4.5 Full‑Sync Protocol
- **Initial Sync Message**: Schema TOML + full snapshot + current version
- **Chunked Transfer**: Split large snapshot into chunks for slow connections
- **Progress Reporting**: Send progress updates during initial sync
- **Client Ready State**: Client signals when it has applied snapshot and is ready for deltas

### 4.6 Incremental Sync
- **Delta Batching**: Group multiple deltas into single network packet
- **Version‑Based Sequencing**: Clients request deltas from a specific version onward
- **Catch‑Up Mechanism**: If client falls behind, send batched deltas from archive
- **Heartbeat & Keepalive**: Detect stalled clients and reconnect

### 4.7 Client Library
- **ClientDB**: Lightweight in‑memory database that mirrors server state
- **Apply Delta**: Apply incoming deltas to local buffers
- **Local Writes**: Optional client‑side writes that are queued and sent to server
- **Event Subscription**: Register callbacks for specific table/entity changes

### 4.8 Integration with Dashboard
- **Replication Dashboard UI**: Show connected clients, sync status, network stats
- **Manual Sync Trigger**: UI button to force full sync to a client
- **Conflict Viewer**: Display conflicts and resolution options
- **Network Log**: Real‑time log of replication events

### 4.9 Testing & Simulation
- **Multi‑Client Tests**: Run server with 10+ simulated clients, verify eventual consistency
- **Network Partition Tests**: Disconnect clients, resume, verify catch‑up
- **Conflict Simulation**: Generate concurrent writes, verify resolution
- **Performance Benchmarks**: Measure replication latency and throughput

## Acceptance Criteria
1. Client can connect, receive full sync, and then incremental updates
2. Delta serialization/deserialization round‑trip lossless
3. Broadcast delivers deltas to all connected clients within 10ms under low load
4. Conflict resolution strategies work as configured; no data corruption
5. Client library can be used standalone (e.g., in a game client) with minimal dependencies
6. Dashboard UI shows client connections and sync status
7. Network partitions are handled; clients catch up after reconnection
8. All replication features work over both TCP and WebSocket (for browser clients)

## Output Artifacts
- Binary delta protocol specification
- Server‑side replication module with broadcast queue
- Client library crate (`ecsdb_client`)
- Example multiplayer game using replication
- Replication dashboard UI components
- Network simulation test suite

## Notes
- Prioritize low latency for game use cases
- Consider using QUIC or WebRTC for future real‑time requirements
- Ensure client library is `no_std` compatible for embedded game consoles (optional)
- Document protocol for third‑party client implementations
