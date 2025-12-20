//! Table schema, field definitions, and relation management.

mod field;
mod query;
mod relation;
#[allow(clippy::module_inception)]
mod table;
pub(crate) mod validation;

pub use field::Field;
pub use relation::Relation;
pub use table::Table;

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
