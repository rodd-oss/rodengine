# Phase 5: Dashboard & Polish
**Estimated Time:** Weeks 17-20

## Overview
Build a user‑friendly dashboard for schema editing, data inspection, and query building. Polish the entire system with comprehensive testing, profiling, documentation, and release packaging.

## Dependencies
- Phase 4 completed (replication) – dashboard can monitor client connections
- Tauri 2 + Vue 3 frontend already set up

## Subtasks

### 5.1 Tauri 2 App Skeleton
- **Window Layout**: Main window with sidebar (schema, data, queries, replication, logs)
- **Routing**: Vue Router for switching between views
- **State Management**: Pinia store for app state (current schema, selected table, etc.)
- **Theme Support**: Light/dark mode toggle

### 5.2 Schema Editor UI
- **Schema Tree View**: Expandable list of tables, fields, custom types, enums
- **Table Editor**: Add/remove tables, modify fields (name, type, constraints)
- **Field Property Panel**: Edit field properties (nullable, indexed, foreign key, etc.)
- **Live Validation**: Validate schema changes in real‑time; show errors
- **Import/Export**: Load schema from TOML file, save to TOML, generate Rust types

### 5.3 Data Viewer Component
- **Table Selection**: Dropdown to choose a component table
- **Paginated Grid**: Display rows (entities) and columns (fields) with virtual scrolling
- **Inline Editing**: Double‑click cell to edit value (type‑aware input)
- **Filter & Sort**: Filter rows by column values, sort by column
- **Bulk Operations**: Select multiple rows for delete, export as CSV/JSON

### 5.4 Query Builder UI
- **Visual Query Builder**: Drag‑and‑drop tables, select fields, set filter conditions
- **Join Builder**: Link tables via foreign keys
- **Query Preview**: Show generated Rust code or pseudo‑SQL
- **Execute Query**: Run query against live database, display results in grid
- **Save/Load Queries**: Persist frequently used queries

### 5.5 Replication Dashboard
- **Client List**: Connected clients with IP, version, last heartbeat, lag
- **Delta Monitor**: Real‑time stream of deltas being broadcast
- **Conflict Viewer**: List recent conflicts and resolutions
- **Manual Sync Controls**: Buttons to force full sync, disconnect client, etc.

### 5.6 Integration Testing
- **End‑to‑End UI Tests**: Playwright/Cypress tests for dashboard workflows
- **Schema Editor Tests**: Create schema, edit fields, save, load
- **Data Interaction Tests**: Insert, update, delete rows via UI
- **Replication UI Tests**: Connect client, verify sync, simulate conflicts

### 5.7 Performance Profiling
- **Benchmark Dashboard**: UI to run built‑in benchmarks, display charts
- **Memory Profiling**: Show buffer sizes, allocation counts, fragmentation
- **Latency Metrics**: Measure read/write/replication latency over time
- **Profile Guided Optimization**: Use profiling data to optimize hot paths

### 5.8 Documentation
- **API Documentation**: `cargo doc` with examples for all public modules
- **User Guide**: How to define schema, write queries, set up replication
- **Architecture Overview**: High‑level diagrams and design rationale
- **Example Projects**: Complete game examples (single‑player, multiplayer)
- **CHANGELOG**: Keep detailed changelog for each release

### 5.9 Release Packaging
- **Crates.io**: Publish `ecsdb` and `ecsdb_client` crates
- **Tauri Bundle**: Package dashboard as standalone desktop app (Windows, macOS, Linux)
- **Docker Image**: Server‑only image for headless deployment
- **Install Scripts**: One‑line install for development setup

### 5.10 Polish & Bug Fixes
- **Error Messages**: User‑friendly error messages with suggested fixes
- **UI/UX Improvements**: Keyboard shortcuts, confirm dialogs, loading indicators
- **Accessibility**: ARIA labels, keyboard navigation, screen reader support
- **Internationalization**: Prepare for i18n (extract strings, placeholder)

## Acceptance Criteria
1. Dashboard loads and displays schema, data, and replication status without errors
2. Schema editor can create/edit/delete tables and fields; changes are validated
3. Data viewer shows component tables with pagination, filtering, and inline editing
4. Query builder can construct simple queries and execute them
5. Replication dashboard shows connected clients and delta activity
6. End‑to‑end UI tests pass for core workflows
7. Performance profiling identifies bottlenecks; optimizations applied
8. Comprehensive documentation published (API docs, user guide, examples)
9. Crates published to crates.io; Tauri app builds for all target platforms

## Output Artifacts
- Tauri dashboard application (bundled executables)
- API documentation website (via `cargo doc`)
- User guide (Markdown in repo, maybe hosted)
- Example game projects in `/examples`
- Published crates `ecsdb` and `ecsdb_client`
- Docker image for server deployment

## Notes
- Prioritize UX for game developers (the primary audience)
- Ensure dashboard works well with large schemas (100+ tables) and large datasets (millions of entities)
- Keep documentation up‑to‑date with each release
- Consider open‑source licensing (MIT/Apache 2.0)
