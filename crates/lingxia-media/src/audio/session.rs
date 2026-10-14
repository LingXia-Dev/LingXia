//! The platform session as a pure function of the live participants.

use super::protocol::{Command, SessionOutput};
use super::types::{Focus, OutputRequest, RouteOutput};

/// What one participant asks of the session while it is live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requirement {
    pub focus: Focus,
    pub obey_silent_switch: bool,
    pub output: OutputRequest,
    pub proximity: bool,
    /// Start order; the most recent non-`mix` participant owns the output.
    pub started: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSpec {
    pub active: bool,
    pub focus: Option<Focus>,
    pub obey_silent_switch: bool,
    pub output: SessionOutput,
    pub proximity: bool,
}

impl SessionSpec {
    pub const INACTIVE: SessionSpec = SessionSpec {
        active: false,
        focus: None,
        obey_silent_switch: true,
        output: SessionOutput::Speaker,
        proximity: false,
    };

    pub fn command(self) -> Command {
        Command::Session {
            active: self.active,
            focus: self.focus,
            obey_silent_switch: self.obey_silent_switch,
            output: self.output,
            proximity: self.proximity,
        }
    }
}

/// Strongest requirement wins: earpiece output > exclusive > duck > mix. A
/// near proximity reading moves the output owner to the earpiece, but only on
/// the built-in route; external devices always win.
pub fn compute<'a>(
    live: impl IntoIterator<Item = &'a Requirement>,
    route: RouteOutput,
    proximity_near: bool,
) -> SessionSpec {
    let mut focus: Option<Focus> = None;
    let mut obey = true;
    let mut owner: Option<&Requirement> = None;
    for requirement in live {
        focus = Some(match focus {
            Some(current) => current.stronger(requirement.focus),
            None => requirement.focus,
        });
        obey &= requirement.obey_silent_switch;
        if requirement.focus != Focus::Mix
            && owner.is_none_or(|current| requirement.started > current.started)
        {
            owner = Some(requirement);
        }
    }
    let Some(focus) = focus else {
        return SessionSpec::INACTIVE;
    };
    let built_in = route.is_built_in();
    let proximity = built_in && owner.is_some_and(|owner| owner.proximity);
    let earpiece = owner.is_some_and(|owner| {
        owner.output == OutputRequest::Earpiece || (proximity && proximity_near)
    });
    SessionSpec {
        active: true,
        focus: Some(if earpiece {
            focus.stronger(Focus::Duck)
        } else {
            focus
        }),
        obey_silent_switch: obey,
        output: if earpiece {
            SessionOutput::Earpiece
        } else {
            SessionOutput::Speaker
        },
        proximity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(focus: Focus, started: u64) -> Requirement {
        Requirement {
            focus,
            obey_silent_switch: focus == Focus::Mix,
            output: OutputRequest::Auto,
            proximity: false,
            started,
        }
    }

    #[test]
    fn nothing_live_deactivates() {
        assert_eq!(
            compute([], RouteOutput::Speaker, false),
            SessionSpec::INACTIVE
        );
    }

    #[test]
    fn strongest_focus_wins_and_silent_switch_needs_every_participant() {
        let mix = req(Focus::Mix, 1);
        let duck = req(Focus::Duck, 2);
        let spec = compute([&mix], RouteOutput::Speaker, false);
        assert_eq!(spec.focus, Some(Focus::Mix));
        assert!(spec.obey_silent_switch);
        let spec = compute([&mix, &duck], RouteOutput::Speaker, false);
        assert_eq!(spec.focus, Some(Focus::Duck));
        assert!(!spec.obey_silent_switch);
        let exclusive = req(Focus::Exclusive, 3);
        assert_eq!(
            compute([&exclusive, &mix, &duck], RouteOutput::Speaker, false).focus,
            Some(Focus::Exclusive)
        );
    }

    #[test]
    fn most_recent_non_mix_participant_owns_the_output() {
        let mut voice = req(Focus::Duck, 2);
        voice.output = OutputRequest::Earpiece;
        let music = req(Focus::Exclusive, 1);
        let spec = compute([&music, &voice], RouteOutput::Speaker, false);
        assert_eq!(spec.output, SessionOutput::Earpiece);
        assert_eq!(spec.focus, Some(Focus::Exclusive));
        let later_music = req(Focus::Exclusive, 3);
        let spec = compute([&voice, &later_music], RouteOutput::Speaker, false);
        assert_eq!(spec.output, SessionOutput::Speaker);
        // A later mix participant never takes the output.
        let mix = req(Focus::Mix, 9);
        let spec = compute([&voice, &mix], RouteOutput::Speaker, false);
        assert_eq!(spec.output, SessionOutput::Earpiece);
        assert_eq!(spec.focus, Some(Focus::Duck));
    }

    #[test]
    fn proximity_applies_only_on_the_built_in_route() {
        let mut voice = req(Focus::Duck, 1);
        voice.proximity = true;
        let far = compute([&voice], RouteOutput::Speaker, false);
        assert!(far.proximity);
        assert_eq!(far.output, SessionOutput::Speaker);
        let near = compute([&voice], RouteOutput::Speaker, true);
        assert_eq!(near.output, SessionOutput::Earpiece);
        let headset = compute([&voice], RouteOutput::Headphones, true);
        assert!(!headset.proximity);
        assert_eq!(headset.output, SessionOutput::Speaker);
    }
}
