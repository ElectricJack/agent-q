//! Shared bounded terminal/history owner lanes across nominal profiles.
use crate::error::AppError;
use tokio::sync::{Semaphore, SemaphorePermit};
static WORK: Semaphore = Semaphore::const_new(2);
static CONTROL: Semaphore = Semaphore::const_new(2);
static HISTORY: Semaphore = Semaphore::const_new(2);
pub(in crate::profiled_execution) fn admit_work() -> Result<SemaphorePermit<'static>, AppError> {
    WORK.try_acquire()
        .map_err(|_| AppError::Maintenance("phase terminal work capacity unavailable".into()))
}
pub(in crate::profiled_execution) fn admit_control() -> Result<SemaphorePermit<'static>, AppError> {
    CONTROL
        .try_acquire()
        .map_err(|_| AppError::Maintenance("phase terminal control capacity unavailable".into()))
}
pub(in crate::profiled_execution) fn admit_history() -> Result<SemaphorePermit<'static>, AppError> {
    HISTORY
        .try_acquire()
        .map_err(|_| AppError::Maintenance("phase history capacity unavailable".into()))
}
