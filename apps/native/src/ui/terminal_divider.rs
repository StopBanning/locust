//! The seam either side of what was typed in a runtime's own terminal (Electron `TerminalDivider.tsx`, 0.391). Those
//! turns came back from the runtime's own record of the session, so they are the person's words and the runtime's
//! answers -- but Locust did not run them, and the runtime ran them with its own permissions. A boundary with a label,
//! as a handoff is, not another thing an agent produced.

use super::*;
use locust_core::conversation_turns::TerminalSeam;

/// The rule's words: its title, and the note under it when there is one.
pub(super) fn words(seam: &TerminalSeam) -> (String, Option<String>) {
    match seam {
        TerminalSeam::Back => ("Back in Locust".into(), None),
        TerminalSeam::Into(runtime) => {
            let name = locust_core::inspector::runtime_display_name(runtime);
            (
                format!("In {name}'s terminal"),
                Some(format!("Typed there and brought back. {name} ran it with its own permissions, not this conversation's mode.")),
            )
        }
    }
}

pub(super) fn view(seam: &TerminalSeam, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let (title, note) = words(seam);
    let mut heading = row().gap(6.0).center_y();
    if let TerminalSeam::Into(runtime) = seam
        && let Some(mark) = crate::look::controls::agent_mark(&l, runtime, 11.0, l.c("text-primary"))
    {
        heading = heading.child(mark);
    }
    heading = heading.child(text(title.clone()).font(l.ty.label).color(l.c("text-primary")));
    let mut body = col().gap(2.0).child(heading);
    if let Some(note) = note {
        body = body.child(text(note).font(l.ty.label).color(l.c("text-muted")).wrap());
    }
    let rule = || el().grow(1.0).h(1.0).fill(p.hairline);
    row()
        .w_full()
        .gap(10.0)
        .center_y()
        .role("separator")
        .label(title)
        .child(rule())
        .child(body.shrink())
        .child(rule())
}
