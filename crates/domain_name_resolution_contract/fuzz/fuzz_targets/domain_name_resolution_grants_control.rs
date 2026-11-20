//! Fuzz `Grants::control` over any sequence of control messages.
//!
//! The input is read 17 bytes at a time: a shape byte, then the two words of one message. When the
//! shape's low bit is clear the words go in exactly as given. When it is set, the operation is
//! forced to a grant or a forget and the badge to 1 through 12 (more badges than there are windows,
//! so the table fills), so the input reaches the paths past the operation check without guessing an
//! opcode. When the shape's second bit is also set, the message is replaced by the whole of a well
//! formed grant of one of five zones, so completed grants are common.
//!
//! Nothing may panic, and after every message the table keeps the rules its documentation states:
//! a window it hands back is below `MAXIMUM_GRANTS`; a badge keeps the window it was granted until
//! that badge is forgotten, and no other badge is granted that window meanwhile; `check` answers
//! `Ok` only for a badge whose grant is complete, with that grant's window; a refusal, a half-sent
//! grant and a forget leave the badge with no grant in force; and each result's reply reads back
//! through `control_answer` as the same acceptance or the same refusal.

#![no_main]

use domain_name_resolution_contract::{
    Control, Denied, Grants, MAXIMUM_GRANTS, OPERATION_FORGET, OPERATION_GRANT, Refusal,
    control_answer, control_fields, control_word, grant_messages,
};
use domain_name_system::Name;
use libfuzzer_sys::fuzz_target;

const MESSAGE_BYTES: usize = 17;
const BADGES: u32 = 12;
const ZONES: [&[u8]; 5] = [b"", b"test", b"a.test", b"b.test", b"a.test."];

/// What the fuzz target has seen the table promise: which badge holds each window.
struct Seen {
    holder: [Option<u32>; MAXIMUM_GRANTS],
}

impl Seen {
    fn window_of(&self, badge: u32) -> Option<usize> {
        self.holder.iter().position(|&h| h == Some(badge))
    }
}

fuzz_target!(|data: &[u8]| {
    let asked = Name::from_dotted("a.test").unwrap();
    let mut grants = Grants::new();
    let mut seen = Seen {
        holder: [None; MAXIMUM_GRANTS],
    };
    for chunk in data.as_chunks::<MESSAGE_BYTES>().0 {
        let shape = chunk[0];
        let first = u64::from_le_bytes(chunk[1..9].try_into().unwrap());
        let second = u64::from_le_bytes(chunk[9..17].try_into().unwrap());
        if shape & 0b11 == 0b11 {
            let badge = (first as u32) % BADGES + 1;
            let zone = ZONES[(second % ZONES.len() as u64) as usize];
            for (first, second) in grant_messages(badge, zone).unwrap() {
                step(&mut grants, &mut seen, &asked, first, second);
            }
        } else if shape & 1 == 1 {
            let (_, offset, total, badge) = control_fields(first);
            let operation = if shape & 0b100 == 0 {
                OPERATION_GRANT
            } else {
                OPERATION_FORGET
            };
            let badge = badge % BADGES + 1;
            step(
                &mut grants,
                &mut seen,
                &asked,
                control_word(operation, offset, total, badge),
                second,
            );
        } else {
            step(&mut grants, &mut seen, &asked, first, second);
        }
    }
});

fn step(grants: &mut Grants, seen: &mut Seen, asked: &Name, first: u64, second: u64) {
    let (_, _, _, badge) = control_fields(first);
    let result = grants.control(first, second);

    match (result, control_answer(result.reply().word())) {
        (Control::Refused(why), Err(Some(answer))) => assert_eq!(why, answer),
        (Control::Refused(_), answer) => panic!("a refusal read back as {answer:?}"),
        (_, Ok(())) => {}
        (accepted, answer) => panic!("{accepted:?} read back as {answer:?}"),
    }

    match result {
        Control::Granted(window) => {
            assert!(window < MAXIMUM_GRANTS);
            if let Some(before) = seen.window_of(badge) {
                assert_eq!(before, window, "a re-grant moved badge {badge}'s window");
            }
            if let Some(other) = seen.holder[window] {
                assert_eq!(
                    other, badge,
                    "window {window} granted while badge {other} holds it"
                );
            }
            seen.holder[window] = Some(badge);
            assert_eq!(grants.granted(badge), Some(window));
        }
        Control::Forgotten(window) => {
            assert!(window < MAXIMUM_GRANTS);
            if let Some(before) = seen.window_of(badge) {
                assert_eq!(
                    before, window,
                    "a forget freed a window badge {badge} does not hold"
                );
            }
            if let Some(other) = seen.holder[window] {
                assert_eq!(
                    other, badge,
                    "badge {badge}'s forget freed badge {other}'s window"
                );
            }
            seen.holder[window] = None;
            assert_eq!(grants.granted(badge), None);
        }
        Control::Partial => assert_eq!(grants.granted(badge), None),
        Control::Refused(Refusal::OutOfOrder | Refusal::TooLong | Refusal::BadZone) => {
            assert_eq!(grants.granted(badge), None);
        }
        Control::Refused(
            Refusal::UnknownOperation | Refusal::BadgeZero | Refusal::Full | Refusal::NoSuchGrant,
        ) => {}
    }

    for probe in 0..=BADGES + 1 {
        let in_force = grants.granted(probe);
        if let Some(window) = in_force {
            assert!(window < MAXIMUM_GRANTS);
        }
        if let (Some(window), Some(at)) = (in_force, seen.window_of(probe)) {
            assert_eq!(
                window, at,
                "badge {probe}'s grant is in force in another window"
            );
        }
        match grants.check(probe, asked) {
            Ok(window) => assert_eq!(in_force, Some(window)),
            Err(Denied::NoGrant) => assert_eq!(in_force, None),
            Err(Denied::OutsideZone) => assert!(in_force.is_some()),
        }
    }
}
