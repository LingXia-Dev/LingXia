//! GPU page automation lives in the shared lower half (`lxapp::automation::gpu`),
//! so `lx.automation` and `lxdev` reach GPU pages the same way.
pub use lxapp::automation::gpu::{GpuAction, GpuFuture, GpuPageAutomation, register};
