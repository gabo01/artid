//! The ops model holds the details for all the big operations performed by artid.
//!
//! This modules comes with two elements:
//!
//!  - A core ops::core that holds the building blocks for most of the operations
//!  - The specific operations ops::{name of the op}

#[cfg(test)]
#[macro_use]
mod test_helpers;

pub mod backup;
pub mod core;
pub mod restore;

pub use self::core::{Error, ErrorKind};

/// Model is a trait for big operations that need to be planned before being executed.
/// Examples of this are operations involving big changes on the filesystem.
///
/// Artid's operations such as backup, restore or zip should be modelled before being
/// executed to allow things such as a dry-run or to cleanup the remnants of a failed
/// operation.
pub trait Model {
    /// The singular action that can be executed by a model. Splitting a model
    /// into small actions allows for easy progress control or to track specific error causes.
    type Action;

    /// The error that can be returned during a model execution
    type Error: std::error::Error;

    /// Performs a standard execution of the model.
    fn run(self) -> Result<(), Self::Error>;

    /// Records the model without executing it. The way the model is logged depends on the
    /// given logger function. The model is logged as a set of singular actions.
    fn log<L: for<'a> Fn(&'a Self::Action)>(&self, logger: &L);

    /// Records the model while it is being executed. This is specially good for tracking
    /// progress on the execution.
    fn log_run<L>(self, logger: &L) -> Result<(), Self::Error>
    where
        L: for<'a> Fn(&'a Self::Action);
}

/// Defines an operation to be performed. An operation is defined as an abstract concept that
/// can be translated to a specific action model.
pub trait Operation {}

/// An operator is the one with the ability to implement an operation. The ability to implement
/// an operation is decided based on the data holded by the object.
pub trait Operator<'mo, O: Operation> {
    /// The builded model
    type Model: Model + 'mo;

    /// The error that can happeng during a model building
    type Error: std::error::Error;

    /// Modifiers options for the model built
    type Options;

    /// Translates the abstract operation to a specific model.
    fn modelate(&'mo mut self, options: Self::Options) -> Result<Self::Model, Self::Error>;
}
