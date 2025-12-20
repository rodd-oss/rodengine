//! Procedure registry and parallel execution.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

pub use in_mem_db_core::database::ProcedureFn;
use in_mem_db_core::error::DbError;

use serde_json::Value;

/// Parameter type for validation.
#[derive(Debug, Clone)]
pub enum ParamType {
    String,
    Number,
    Boolean,
    Array,
    Object,
    Any,
}

/// Returns a string representation of a JSON value's type.
fn value_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Parameter requirement for procedure validation.
#[derive(Debug, Clone)]
pub struct ParamRequirement {
    /// Parameter name.
    pub name: String,
    /// Expected type.
    pub param_type: ParamType,
    /// Whether the parameter is required.
    pub required: bool,
}

/// Schema for procedure parameter validation.
#[derive(Debug, Clone)]
pub struct ProcedureSchema {
    /// List of parameter requirements.
    pub params: Vec<ParamRequirement>,
}

impl ProcedureSchema {
    /// Validates JSON parameters against this schema.
    pub fn validate(&self, params: &Value) -> Result<(), DbError> {
        let obj = params.as_object().ok_or_else(|| DbError::TypeMismatch {
            expected: "object".to_string(),
            got: value_type(params).to_string(),
        })?;

        for req in &self.params {
            let value = obj.get(&req.name);
            match value {
                Some(v) => {
                    // Check type
                    match req.param_type {
                        ParamType::String if !v.is_string() => {
                            return Err(DbError::TypeMismatch {
                                expected: "string".to_string(),
                                got: value_type(v).to_string(),
                            });
                        }
                        ParamType::Number if !v.is_number() => {
                            return Err(DbError::TypeMismatch {
                                expected: "number".to_string(),
                                got: value_type(v).to_string(),
                            });
                        }
                        ParamType::Boolean if !v.is_boolean() => {
                            return Err(DbError::TypeMismatch {
                                expected: "boolean".to_string(),
                                got: value_type(v).to_string(),
                            });
                        }
                        ParamType::Array if !v.is_array() => {
                            return Err(DbError::TypeMismatch {
                                expected: "array".to_string(),
                                got: value_type(v).to_string(),
                            });
                        }
                        ParamType::Object if !v.is_object() => {
                            return Err(DbError::TypeMismatch {
                                expected: "object".to_string(),
                                got: value_type(v).to_string(),
                            });
                        }
                        ParamType::Any => {}
                        _ => {}
                    }
                }
                None => {
                    if req.required {
                        return Err(DbError::FieldNotFound {
                            table: "procedure".to_string(),
                            field: req.name.clone(),
                        });
                    }
                }
            }
        }

        Ok(())
    }
}

/// Internal representation of a registered procedure.
struct ProcedureDef {
    /// Function pointer.
    func: ProcedureFn,
    /// Optional schema for parameter validation.
    schema: Option<ProcedureSchema>,
}

/// Registry of available procedures.
#[derive(Default, Clone)]
pub struct ProcedureRegistry {
    procedures: Arc<RwLock<HashMap<String, ProcedureDef>>>,
}

impl ProcedureRegistry {
    /// Creates a new empty procedure registry.
    pub fn new() -> Self {
        Self {
            procedures: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a procedure with the given name.
    ///
    /// # Arguments
    /// * `name` - Procedure name
    /// * `func` - Procedure function
    pub fn register(&mut self, name: String, func: ProcedureFn) {
        self.procedures
            .write()
            .insert(name, ProcedureDef { func, schema: None });
    }

    /// Registers a procedure with a schema for parameter validation.
    ///
    /// # Arguments
    /// * `name` - Procedure name
    /// * `func` - Procedure function
    /// * `schema` - Schema for parameter validation
    pub fn register_with_schema(
        &mut self,
        name: String,
        func: ProcedureFn,
        schema: ProcedureSchema,
    ) {
        self.procedures.write().insert(
            name,
            ProcedureDef {
                func,
                schema: Some(schema),
            },
        );
    }

    /// Gets a procedure function by name.
    ///
    /// # Arguments
    /// * `name` - Procedure name
    ///
    /// # Returns
    /// `Option<ProcedureFn>` containing the procedure if found.
    pub fn get(&self, name: &str) -> Option<ProcedureFn> {
        self.procedures.read().get(name).map(|def| def.func)
    }

    /// Validates parameters for a procedure.
    ///
    /// # Arguments
    /// * `name` - Procedure name
    /// * `params` - JSON parameters to validate
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or validation failure.
    pub fn validate_params(&self, name: &str, params: &Value) -> Result<(), DbError> {
        let procedures = self.procedures.read();
        let def = procedures
            .get(name)
            .ok_or_else(|| DbError::ProcedureNotFound {
                name: name.to_string(),
            })?;

        if let Some(schema) = &def.schema {
            schema.validate(params)?;
        }
        Ok(())
    }
}
