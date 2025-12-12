# Database Technical Reference Document

## 1. Overview

Relational in-memory database for online games. No SQL support; REST API for schema and data operations. Designed for low latency, high throughput, and cache-efficient access.

## 2. Architecture & Components

- **Implementation Language**: Rust (required).
- **Runtime**: Own event loop with configurable tickrate (15–120 Hz). Executes handlers (API calls, custom procedures) each tick.
- **Storage**: In-memory row‑oriented storage (`Vec<u8>` per table) with parallel disk persistence.
- **Concurrency**: Atomic operations via ArcSwap on buffers; lock-free reads and writes.
- **Transaction**: Default atomic transactions; each CRUD operation atomic.
- **Schema**: JSON file for schema definition.

## 3. Data Model

- **Tables**: Named collections of records.
- **Fields**: Named and typed from predefined type list. Supports default DB types and custom types (e.g., `3xf32` for vectors).
- **Relations**: Relations between tables supported for all tables.
- **Records**: Ordered collection of field values.

## 4. Storage Layout

- **Table Storage**: `Vec<u8>`; unsafe casting of defined fields to storage.
- **Zero Copy**: Minimal allocations; zero-copy access.
- **Packing**:
  - Fields within a record tightly packed for CPU cache locality.
  - Records within a table tightly packed for CPU cache locality.
- **Buffer Management**: ArcSwap for parallel read/write.

## 5. Operations & API

REST API endpoints:

- **Schema**: Create/delete tables, fields, relations.
- **CRUD**: Create, read, update, delete records.
- **RPC**: Remote procedure call.
- **Procedures**: Custom transactional procedures that commit at end.

## 6. Performance Requirements

- **Atomicity**: Each CRUD operation atomic without mutex locks.
- **Parallelism**: Parallel read/write operations via ArcSwap.
- **Cache Efficiency**: Tight packing for cache friendliness.
- **Procedural Parallelism**: Procedures can iterate parallel across all CPU cores on single table data to maximize cache hit.
- **Tickrate**: 15–120 Hz runtime loop.

## 7. Integration & Runtime

- **Location**: Rust crate in `packages/` (monorepo workspace).
- **Persistence**: In-memory operations with parallel disk saves.
- **Schema Persistence**: JSON file.
- **Runtime Loop**: Dedicated database loop with tickrate; handlers for API and procedures.

## 8. Implementation Notes

- **Language**: Rust (required implementation language). FFI for other languages optional.
- **Workspace**: Defined in root `Cargo.toml`.
- **Testing**: Use `cargo test`.
- **Linting**: Follow project ESLint/Prettier for any frontend bindings.
- **Type Safety**: Unsafe casting limited to field-to-storage mapping; ensure memory safety via rigorous validation.
- **Concurrency**: Leverage `ArcSwap` crate for lock‑free buffer swaps.
- **Serialization**: JSON for schema; binary format for disk snapshots.
