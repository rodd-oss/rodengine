# Test Plan: Custom Procedure Registration (task_pr_1)

## Overview

Tests for registering custom procedures (function pointers/closures) in the relational in-memory database. Procedures are transactional, execute within the database's event loop, and must handle edge cases around concurrency, memory safety, and error handling.

## Test Cases

### 1. `register_procedure_basic`

**Verifies**: Basic procedure registration with valid function pointer.
**Assertions**:

- Procedure can be registered with unique name
- Registration returns success/OK
- Procedure can be retrieved by name
- Procedure metadata (name, signature) stored correctly

### 2. `register_procedure_duplicate_name`

**Verifies**: Duplicate procedure name handling.
**Assertions**:

- Second registration with same name fails
- Returns appropriate error (e.g., `ProcedureAlreadyExists`)
- Original procedure remains registered and functional

### 3. `register_procedure_with_closure`

**Verifies**: Closure/Fn trait object registration.
**Assertions**:

- Closure capturing environment can be registered
- Closure maintains captured state across invocations
- Type erasure works correctly (Box<dyn Fn(...)>)

### 4. `register_procedure_invalid_signature`

**Verifies**: Signature validation for procedure functions.
**Edge Cases**:

- Function returning wrong type
- Function with incorrect parameter count/types
- Function with non-'static lifetime where required
  **Assertions**: Invalid signatures rejected with clear error

### 5. `register_procedure_transaction_context`

**Verifies**: Procedures receive transaction context.
**Assertions**:

- Procedure receives database handle/transaction context
- Can perform CRUD operations within procedure
- Changes are visible within procedure but not committed until success

### 6. `register_procedure_concurrent_registration`

**Verifies**: Thread-safe procedure registration.
**Edge Cases**:

- Multiple threads registering procedures simultaneously
- Race condition on duplicate name check
  **Assertions**: No data races, all procedures registered correctly

### 7. `unregister_procedure`

**Verifies**: Procedure removal functionality.
**Assertions**:

- Can unregister existing procedure
- Returns success/OK
- Subsequent calls fail with `ProcedureNotFound`
- Memory/resources properly cleaned up

### 8. `list_registered_procedures`

**Verifies**: Procedure enumeration.
**Assertions**:

- Returns list of all registered procedure names
- List is complete and accurate
- Empty list when no procedures registered

### 9. `procedure_execution_with_parameters`

**Verifies**: Parameter passing to procedures.
**Edge Cases**:

- Different parameter types (primitives, strings, custom types)
- Variable number of parameters
- Default parameter values (if supported)
  **Assertions**: Parameters correctly passed and accessible in procedure body

### 10. `procedure_return_values`

**Verifies**: Return value handling.
**Assertions**:

- Procedure can return values
- Return types preserved and accessible
- Error return values handled appropriately

### 11. `procedure_panic_handling`

**Verifies**: Panic safety within procedures.
**Edge Cases**:

- Procedure panics during execution
- Memory leaks from panicked procedures
- Transaction rollback on panic (ties to task_pr_2)
  **Assertions**: Panics don't crash database, resources cleaned up

### 12. `procedure_lifetime_validation`

**Verifies**: Lifetime safety for captured references.
**Edge Cases**:

- Closure capturing temporary references
- Procedure outliving captured data
- 'static lifetime requirement enforcement
  **Assertions**: Compile-time or runtime validation prevents use-after-free

### 13. `procedure_registry_persistence`

**Verifies**: Procedure registry survives database operations.
**Assertions**:

- Procedures remain registered after buffer swaps (ArcSwap)
- Procedures survive schema modifications
- Registry consistent across concurrent reads/writes

### 14. `procedure_max_count_limit`

**Verifies**: Resource limits on procedure count.
**Edge Cases**:

- Registering beyond maximum allowed procedures
- Memory usage with many procedures
  **Assertions**: Clear error when limit reached, no memory exhaustion

### 15. `procedure_name_validation`

**Verifies**: Procedure name validation rules.
**Edge Cases**:

- Empty names
- Names with invalid characters
- Reserved names (e.g., "transaction", "commit")
- Case sensitivity considerations
  **Assertions**: Invalid names rejected with descriptive error

## Key Edge Cases

1. **Duplicate registration** - Prevent overwriting existing procedures
2. **Thread safety** - Concurrent registration/execution
3. **Memory safety** - No use-after-free with captured references
4. **Error handling** - Clear error messages for invalid procedures
5. **Resource limits** - Prevent DoS via unlimited procedure registration
6. **Type safety** - Ensure procedure signatures match expected types
7. **Transaction isolation** - Procedures see consistent database state
8. **Panic safety** - Database remains stable if procedure panics

## Integration Points

- Works with `task_pr_2` (transactional execution)
- Integrates with runtime event loop (`task_el_2`)
- Compatible with parallel execution (`task_pp_1`)
- Respects atomic operations and ArcSwap buffer management
