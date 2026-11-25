pub mod coaches;
pub mod commands;
pub mod document;
pub mod safety;
pub mod synchronization;

#[cfg(test)]
mod synchronization_tests;

#[cfg(test)]
mod coaches_tests;

#[cfg(test)]
pub(crate) mod ats_tests;

#[cfg(test)]
mod persistence_ipc_tests;
