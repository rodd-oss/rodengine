//! HTTP endpoint implementations for CRUD, DDL, and RPC.

mod crud_handlers;
mod ddl_handlers;
mod query_handlers;
mod request_utils;
mod response;
mod rpc_handlers;

// Re-export public items
pub use crud_handlers::{
    create_record, delete_record, partial_update_record, read_record, update_record,
};
pub use ddl_handlers::{
    add_field, create_relation, create_table, delete_relation, delete_table, remove_field,
};
pub use query_handlers::{list_tables, query_records};
pub use request_utils::{
    build_empty_response, build_response, map_db_error_to_router_error,
    read_request_body_with_timeout, wait_for_response_with_timeout, AddFieldRequest,
    AddFieldResponse, CreateRelationRequest, CreateRelationResponse, CreateTableRequest,
    CreateTableResponse, FieldDefinition, MatchitParams,
};
pub use response::{error_response, success_response, ApiError, ApiResponse, ErrorResponse};
pub use rpc_handlers::rpc;

// Re-export request types that are used in handlers
pub use request_utils::{
    CreateRecordRequest, CreateRecordResponse, PartialUpdateRequest, UpdateRecordRequest,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::RouterError;

    #[test]
    fn test_parse_query_params() {
        // Test empty query
        let params = request_utils::parse_query_params(None).unwrap();
        assert!(params.limit.is_none());
        assert!(params.offset.is_none());
        assert!(params.filters.is_empty());

        // Test limit and offset
        let params = request_utils::parse_query_params(Some("limit=10&offset=5")).unwrap();
        assert_eq!(params.limit, Some(10));
        assert_eq!(params.offset, Some(5));
        assert!(params.filters.is_empty());

        // Test field filters
        let params = request_utils::parse_query_params(Some("name=User1&active=true")).unwrap();
        assert!(params.limit.is_none());
        assert!(params.offset.is_none());
        assert_eq!(params.filters.len(), 2);
        assert_eq!(
            params.filters.get("name").unwrap(),
            &serde_json::json!("User1")
        );
        assert_eq!(
            params.filters.get("active").unwrap(),
            &serde_json::json!(true)
        );

        // Test mixed parameters
        let params =
            request_utils::parse_query_params(Some("limit=5&offset=2&active=false&id=42")).unwrap();
        assert_eq!(params.limit, Some(5));
        assert_eq!(params.offset, Some(2));
        assert_eq!(params.filters.len(), 2);
        assert_eq!(
            params.filters.get("active").unwrap(),
            &serde_json::json!(false)
        );
        assert_eq!(params.filters.get("id").unwrap(), &serde_json::json!(42));

        // Test invalid limit
        let result = request_utils::parse_query_params(Some("limit=abc"));
        assert!(result.is_err());

        // Test invalid offset
        let result = request_utils::parse_query_params(Some("offset=xyz"));
        assert!(result.is_err());

        // Test invalid JSON in filter (should be treated as string)
        let params = request_utils::parse_query_params(Some("name={invalid json")).unwrap();
        assert_eq!(
            params.filters.get("name").unwrap(),
            &serde_json::json!("{invalid json")
        );
    }

    #[test]
    fn test_map_db_error_to_router_error() {
        use in_mem_db_core::error::DbError;

        // Test client errors (should map to BadRequest)
        let client_errors = vec![
            DbError::TableNotFound {
                table: "test".to_string(),
            },
            DbError::FieldNotFound {
                table: "test".to_string(),
                field: "id".to_string(),
            },
            DbError::RecordNotFound {
                table: "test".to_string(),
                index: 0,
            },
            DbError::ProcedureNotFound {
                name: "test".to_string(),
            },
            DbError::FieldAlreadyExists {
                table: "test".to_string(),
                field: "id".to_string(),
            },
            DbError::CapacityOverflow { operation: "test" },
            DbError::TypeMismatch {
                expected: "u64".to_string(),
                got: "string".to_string(),
            },
            DbError::InvalidOffset {
                table: "test".to_string(),
                offset: 100,
                max: 50,
            },
            DbError::TableAlreadyExists("test".to_string()),
        ];

        for error in client_errors {
            let router_error = map_db_error_to_router_error(error.clone());
            match router_error {
                RouterError::BadRequest(_) => {
                    // Expected for non-404 errors
                    assert!(
                        error.to_string().contains("test") || error.to_string().contains("u64")
                    );
                    // Make sure it's not a TableNotFound or FieldNotFound
                    match error {
                        DbError::TableNotFound { .. } | DbError::FieldNotFound { .. } => {
                            panic!("Expected NotFound for {:?}, got BadRequest", error)
                        }
                        _ => {} // OK
                    }
                }
                RouterError::NotFound(_) => {
                    // Expected for TableNotFound, FieldNotFound, RecordNotFound, and ProcedureNotFound
                    match error {
                        DbError::TableNotFound { .. }
                        | DbError::FieldNotFound { .. }
                        | DbError::RecordNotFound { .. }
                        | DbError::ProcedureNotFound { .. } => {} // OK
                        _ => panic!("Expected BadRequest for {:?}, got NotFound", error),
                    }
                }
                _ => panic!(
                    "Expected BadRequest or NotFound for {:?}, got {:?}",
                    error, router_error
                ),
            }
        }

        // Test server errors (should map to InternalError)
        let server_errors = vec![
            DbError::SerializationError("test".to_string()),
            DbError::ProcedurePanic("test".to_string()),
            DbError::LockPoisoned,
            DbError::Timeout,
        ];

        for error in server_errors {
            let router_error = map_db_error_to_router_error(error.clone());
            match router_error {
                RouterError::InternalError(_) => {
                    // Expected
                    assert!(router_error.to_string().contains("Runtime error"));
                }
                _ => panic!(
                    "Expected InternalError for {:?}, got {:?}",
                    error, router_error
                ),
            }
        }
    }
}
