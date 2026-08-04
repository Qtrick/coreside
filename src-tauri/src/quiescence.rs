//! Profile-generation-bound quiescence for maintenance / restore.
//!
//! Subsystems consult [`QuiescenceCoordinator`] before starting profile-dependent
//! work. Pause tokens are bound to a generation so a stale resume after a profile
//! bump cannot unpause the new generation.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::commands::CommandError;

/// Profile-dependent subsystems that honor quiescence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuiescedSubsystem {
    QueueDrain,
    PatchScheduler,
    AttachmentGc,
}

/// Token returned by [`QuiescenceCoordinator::pause`]. Resume only succeeds when
/// the token still matches the generation captured at pause and the current
/// profile generation has not been bumped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PauseToken {
    generation: u64,
}

impl PauseToken {
    pub fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Debug, Default)]
pub struct QuiescenceCoordinator {
    /// Monotonic profile generation; bump on successful profile replace/swap.
    generation: AtomicU64,
    paused: AtomicBool,
    /// Generation captured when pause became active.
    pause_generation: AtomicU64,
}

impl QuiescenceCoordinator {
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Advance generation after a profile database replace / restore swap.
    /// Outstanding pause tokens for the previous generation become invalid.
    pub fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Pause registered subsystems. Returns a token for a matching [`Self::resume`].
    pub fn pause(&self) -> PauseToken {
        let gen = self.generation.load(Ordering::SeqCst);
        self.pause_generation.store(gen, Ordering::SeqCst);
        self.paused.store(true, Ordering::SeqCst);
        PauseToken { generation: gen }
    }

    /// Resume only when `token` matches the pause generation and the current
    /// profile generation is unchanged since pause.
    pub fn resume(&self, token: PauseToken) -> bool {
        if !self.paused.load(Ordering::SeqCst) {
            return false;
        }
        let pause_gen = self.pause_generation.load(Ordering::SeqCst);
        let current = self.generation.load(Ordering::SeqCst);
        if token.generation != pause_gen || token.generation != current {
            return false;
        }
        self.paused.store(false, Ordering::SeqCst);
        true
    }

    /// Clear pause regardless of token (maintenance clear / forced recovery exit).
    pub fn force_resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn allows(&self, _subsystem: QuiescedSubsystem) -> bool {
        !self.is_paused()
    }

    pub fn require_active(&self, subsystem: QuiescedSubsystem) -> Result<(), CommandError> {
        if self.allows(subsystem) {
            Ok(())
        } else {
            let _ = subsystem;
            Err(CommandError::new(
                "maintenance_in_progress",
                "Coreside is updating your profile. Try again when maintenance finishes.",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_resume_with_matching_token() {
        let q = QuiescenceCoordinator::default();
        assert!(!q.is_paused());
        assert!(q.allows(QuiescedSubsystem::QueueDrain));
        let token = q.pause();
        assert_eq!(token.generation(), 0);
        assert!(q.is_paused());
        assert!(!q.allows(QuiescedSubsystem::PatchScheduler));
        assert!(!q.allows(QuiescedSubsystem::AttachmentGc));
        assert!(q.resume(token));
        assert!(!q.is_paused());
        assert!(q.allows(QuiescedSubsystem::QueueDrain));
    }

    #[test]
    fn resume_ignored_after_generation_bump() {
        let q = QuiescenceCoordinator::default();
        let token = q.pause();
        assert_eq!(q.bump_generation(), 1);
        assert!(q.is_paused());
        assert!(!q.resume(token));
        assert!(q.is_paused());
        q.force_resume();
        assert!(!q.is_paused());
    }

    #[test]
    fn stale_token_cannot_resume_new_pause() {
        let q = QuiescenceCoordinator::default();
        let stale = q.pause();
        q.force_resume();
        q.bump_generation();
        let _fresh = q.pause();
        assert!(!q.resume(stale));
        assert!(q.is_paused());
    }

    #[test]
    fn require_active_errors_while_paused() {
        let q = QuiescenceCoordinator::default();
        q.pause();
        let err = q
            .require_active(QuiescedSubsystem::QueueDrain)
            .expect_err("paused");
        assert_eq!(err.code, "maintenance_in_progress");
    }
}
