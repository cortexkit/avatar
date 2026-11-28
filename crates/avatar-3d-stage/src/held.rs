//! Face commands held until their face set can be drawn.
//!
//! A `set_state` that changes the mood, or a `play` whose reaction shows a
//! face, needs a face set. When that set is not resident yet, the command
//! waits here instead of reaching the actor, so the face it asks for never
//! has to be drawn before it exists. Each key keeps the host's order: once
//! a key has a command waiting, every later `set_state` or `play` for it
//! waits behind.

use std::collections::VecDeque;

use avatar_3d::motion::{ActorState, Reaction, StatusLoop};
use avatar_3d::robot::Robot;
use avatar_small_robots::Mood;

use crate::faces::HOLD_AT_MOST;

/// A face command for one key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FaceCommand {
    SetState {
        mood: Mood,
        status_loop: Option<StatusLoop>,
    },
    Play(Reaction),
}

impl FaceCommand {
    /// The face set the command needs on `robot` driven by `actor`: the
    /// new mood's for a mood change, the reaction's face for a reaction
    /// that shows one, else none.
    pub fn needs(&self, robot: &Robot, actor: &ActorState) -> Option<Mood> {
        match *self {
            Self::SetState { mood, .. } => {
                let current = actor
                    .mood_changes()
                    .last()
                    .map_or(actor.base(), |&(_, m)| m);
                (mood != current).then_some(mood)
            }
            Self::Play(reaction) => robot.motion.reaction(reaction).face,
        }
    }

    /// Gives the command to the actor at `t`.
    pub fn apply(&self, actor: &mut ActorState, t: f64) {
        match *self {
            Self::SetState { mood, status_loop } => {
                actor.set_mood(t, mood);
                actor.set_loop(t, status_loop);
            }
            Self::Play(reaction) => actor.play(t, reaction),
        }
    }

    fn same_kind(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::SetState { .. }, Self::SetState { .. }) | (Self::Play(_), Self::Play(_))
        )
    }
}

/// A command waiting, and the stage time the frame thread took it at.
#[derive(Clone, Copy, Debug)]
struct Waiting {
    command: FaceCommand,
    drained: f64,
}

/// One key's held commands, in the host's order.
#[derive(Debug, Default)]
pub(crate) struct Held {
    queue: VecDeque<Waiting>,
}

impl Held {
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Holds `command`, taken at `t`: it replaces any waiting command of
    /// its kind (a `set_state` its mood and status loop together) and joins
    /// the tail.
    pub fn hold(&mut self, command: FaceCommand, t: f64) {
        self.queue.retain(|w| !w.command.same_kind(&command));
        self.queue.push_back(Waiting {
            command,
            drained: t,
        });
    }

    /// The commands waiting, in order.
    pub fn commands(&self) -> impl Iterator<Item = &FaceCommand> {
        self.queue.iter().map(|w| &w.command)
    }

    /// Takes, in order, the commands due on the drawn frame at `t`: the
    /// head once its set is drawable or it has waited [`HOLD_AT_MOST`]
    /// seconds of stage time, then each one behind it on the same terms,
    /// stopping at the first that is not due.
    pub fn due(&mut self, t: f64, drawable: impl Fn(&FaceCommand) -> bool) -> Vec<FaceCommand> {
        let mut due = Vec::new();
        while let Some(head) = self.queue.front() {
            if !(drawable(&head.command) || t - head.drained >= HOLD_AT_MOST) {
                break;
            }
            due.push(head.command);
            self.queue.pop_front();
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: FaceCommand = FaceCommand::SetState {
        mood: Mood::Happy,
        status_loop: None,
    };
    const B: FaceCommand = FaceCommand::SetState {
        mood: Mood::Sad,
        status_loop: Some(StatusLoop::Working),
    };
    const R: FaceCommand = FaceCommand::Play(Reaction::Hello);
    const S: FaceCommand = FaceCommand::Play(Reaction::Ask);

    #[test]
    fn a_later_command_replaces_its_kind_and_joins_the_tail() {
        let mut held = Held::default();
        held.hold(A, 0.0);
        held.hold(R, 0.1);
        held.hold(B, 0.2);
        assert_eq!(held.commands().copied().collect::<Vec<_>>(), [R, B]);
        held.hold(S, 0.3);
        assert_eq!(held.commands().copied().collect::<Vec<_>>(), [B, S]);
    }

    #[test]
    fn commands_come_due_in_order_by_set_or_by_timer() {
        let mut held = Held::default();
        held.hold(R, 1.0);
        held.hold(B, 1.2);
        // Neither set drawable: nothing before R's timer runs out.
        assert!(held.due(1.49, |_| false).is_empty());
        // R is due by its timer, and B stops the queue: its own timer runs
        // from its own drain.
        assert_eq!(held.due(1.5, |_| false), [R]);
        assert!(held.due(1.69, |_| false).is_empty());
        assert_eq!(held.due(1.71, |_| false), [B]);
        assert!(held.is_empty());

        // A drawable head lets the drawable ones behind it follow.
        held.hold(A, 2.0);
        held.hold(R, 2.0);
        assert_eq!(held.due(2.1, |c| *c == A || *c == R), [A, R]);
        held.hold(A, 3.0);
        held.hold(R, 3.0);
        assert_eq!(held.due(3.1, |c| *c == A), [A]);
        assert_eq!(held.commands().copied().collect::<Vec<_>>(), [R]);
    }
}
