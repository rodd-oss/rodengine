# Weekly Implementation Breakdown

## Week 1-2: Phase 1 - Foundation (`db-core`)

### Week 1

**Day 1**: Workspace Setup

- Update root `Cargo.toml` with workspace configuration
- Create all package directories and `Cargo.toml` files
- Verify `cargo build --workspace` succeeds

**Day 2-3**: Storage Buffer

- Implement `TableBuffer` with `Vec<u8>` storage
- Capacity management: `new_with_capacity`, `reserve`, `shrink_to_fit`
- Tests: `db_test_plan_sl_1.md` (basic initialization, edge capacities)

**Day 4**: Field Types & Record Size

- Define `FieldType` enum in `db-types`
- Calculate record size with tight packing (no padding)
- Tests: `db_test_plan_sl_2.md`, `db_test_plan_sl_3.md`

**Day 5**: Unsafe Pointer Casting

- Implement `write_record` and `read_record` with unsafe casts
- Data integrity validation after reads
- Tests: `db_test_plan_sl_4.md`, `db_test_plan_sl_5.md`

### Week 2

**Day 6**: Zero-copy Access

- Field accessors returning `&T` references
- Record iterator yielding references
- Tests: `db_test_plan_zc_1.md`, `db_test_plan_zc_2.md`

**Day 7**: Memory Safety

- Bounds checking for record and field indices
- Field offset/size validation
- Tests: `db_test_plan_ms_1.md`, `db_test_plan_ms_2.md`

**Day 8-9**: Table Schema

- `Table` struct with name and field definitions
- `Field` struct with name, type, byte offset
- Tests: `db_test_plan_ts_1.md`, `db_test_plan_ts_2.md`

**Day 10**: Schema Operations

- Table creation/destruction in database catalog
- Field addition/removal with validation
- Tests: `db_test_plan_ts_3.md`, `db_test_plan_ts_4.md`

## Week 3-4: Phase 2 - Data Model (`db-types` + `db-core`)

### Week 3

**Day 11-12**: Custom Types

- Built-in scalar types (i32, u64, f32, bool)
- Type registry for user-defined composites
- Tests: `db_test_plan_ct_1.md`, `db_test_plan_ct_2.md`

**Day 13-14**: Relations

- `Relation` struct with source/destination tables
- Referential integrity enforcement on delete
- Tests: `db_test_plan_rl_1.md`, `db_test_plan_rl_2.md`

### Week 4

**Day 15-16**: JSON Schema

- Serialize entire schema to JSON file
- Deserialize and recreate in-memory structures
- Tests: `db_test_plan_sj_1.md`, `db_test_plan_sj_2.md`

**Day 17**: Cache Efficiency

- Verify record packing eliminates padding
- Ensure contiguous `Vec<u8>` allocation
- Tests: `db_test_plan_ce_1.md`, `db_test_plan_ce_2.md`

## Week 5-6: Phase 3 - Concurrency (`db-core`)

### Week 5

**Day 18-19**: ArcSwap Integration

- Wrap table buffer in `Arc<Vec<u8>>`
- Atomic buffer swapping with `ArcSwap`
- Copy-on-write strategy for modifications
- Tests: `db_test_plan_ab_1.md`, `db_test_plan_ab_2.md`, `db_test_plan_ab_3.md`

**Day 20-21**: Atomic Operations

- Ensure CRUD operations are atomic (all-or-nothing)
- Transaction log for rollback on partial failures
- Tests: `db_test_plan_ao_1.md`, `db_test_plan_ao_2.md`

### Week 6

**Day 22-23**: Lock-free Reads

- Read API with `ArcSwap::load` for current buffer
- Readers not blocked by concurrent writers
- Tests: `db_test_plan_lf_1.md`, `db_test_plan_lf_2.md`

## Week 7-8: Phase 4 - Runtime (`db-runtime`)

### Week 7

**Day 24-25**: Event Loop

- Main database loop with configurable tickrate (15-120 Hz)
- Handler registration for API calls and procedures
- Tests: `db_test_plan_el_1.md`, `db_test_plan_el_2.md`

### Week 8

**Day 26-27**: Parallel Procedures

- API for parallel iteration over table records (rayon)
- Cache locality optimization (chunk size aligned to cache line)
- Tests: `db_test_plan_pp_1.md`, `db_test_plan_pp_2.md`

## Week 9-10: Phase 5 - API Layer (`db-api`)

### Week 9

**Day 28-30**: REST Schema Endpoints

- `POST /table` - Create table
- `DELETE /table/{name}` - Delete table
- `POST /table/{name}/field` - Add field
- `DELETE /table/{name}/field/{fieldName}` - Remove field
- `POST /relation` - Create relation
- `DELETE /relation/{id}` - Delete relation
- Tests: `db_test_plan_rs_1.md` through `db_test_plan_rs_6.md`

### Week 10

**Day 31-33**: REST CRUD Endpoints

- `POST /table/{name}/record` - Insert record
- `GET /table/{name}/record/{id}` - Retrieve record
- `PUT /table/{name}/record/{id}` - Update record
- `DELETE /table/{name}/record/{id}` - Delete record
- `GET /table/{name}/records` - List all records (pagination)
- Tests: `db_test_plan_rc_1.md` through `db_test_plan_rc_5.md`

**Day 34-35**: RPC & Procedures

- JSON-RPC over HTTP protocol
- `POST /rpc` endpoint with handler dispatch
- Custom procedure registration and execution
- Transactional execution with auto-commit/rollback
- Tests: `db_test_plan_rp_1.md`, `db_test_plan_rp_2.md`, `db_test_plan_pr_1.md`, `db_test_plan_pr_2.md`

## Week 11-12: Phase 6 - Persistence & Integration

### Week 11

**Day 36-37**: Disk Persistence

- Periodic snapshot of entire database to binary file
- Background thread snapshot without blocking main loop
- Tests: `db_test_plan_ds_1.md`, `db_test_plan_ds_2.md`

**Day 38-39**: Recovery

- Load database from binary snapshot on startup
- Snapshot integrity validation (checksum, version)
- Tests: `db_test_plan_re_1.md`, `db_test_plan_re_2.md`

### Week 12

**Day 40**: Applications

- Create `db-server` main executable with configuration
- Build `db-cli` command-line interface
- Create `db-test-runner` for TDD execution

**Day 41**: Monorepo Integration

- Final workspace configuration
- Cross-package dependency resolution
- Build and test entire workspace
- Tests: `db_test_plan_mi_1.md`, `db_test_plan_mi_2.md`, `db_test_plan_mi_3.md`

**Day 42**: Documentation & Polish

- API documentation
- Usage examples
- Performance benchmarks
- Final validation against all requirements
