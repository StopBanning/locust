//! Settings › General › Help (Electron `Screens.tsx`, the `Help` section, 0.593): where a person reaches Locust's makers
//! -- one private address, the public bug page, and what each carries. Words for words Electron's; the address is
//! `report_problem::SUPPORT_ADDRESS` (shared/support.ts). What it names is in Report a problem above: Send feedback opens
//! the GitHub issue, Email fills the address in, Save as a file keeps a long conversation (`feedback_dialog`).

use super::*;
use locust_core::report_problem::SUPPORT_ADDRESS;

pub(crate) const HEADING: &str = "Help";
const LEDE: &str = "Stuck, or something looks wrong? Two ways to reach the people who make Locust.";

/// The receipt's terms and what each says (`<dl className="lc-receipt">`); the address is drawn in the code face.
pub(crate) const RECEIPT: &[(&str, &str)] = &[
    ("Email", SUPPORT_ADDRESS),
    ("Who reads it", "Only Locust\u{2019}s makers. Send feedback above fills the email in: what you wrote, your Locust version and system, and a conversation only if you include it."),
    ("Bug reports", "Send feedback above also opens a GitHub issue, filled in the same way. Issues are public and need a GitHub account."),
    ("Long reports", "An email cuts off a long conversation. Save as a file keeps all of it; attach the file to the email or the issue."),
];

pub(crate) fn section(_s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let mut receipt = col().w_full().gap(8.0).px(10.0).pb(10.0).key("settings-help-receipt");
    for (term, said) in RECEIPT {
        let face = if *term == "Email" { l.ty.mono } else { l.ty.body };
        receipt = receipt.child(
            row()
                .w_full()
                .gap(12.0)
                .child(text(*term).font(type_::LABEL).color(p.muted).w(120.0))
                .child(col().grow(1.0).child(text(*said).font(face).color(p.secondary).wrap())),
        );
    }
    col()
        .w_full()
        .key("settings-help")
        .child(section_heading(HEADING, p))
        .child(text(LEDE).font(type_::LABEL).color(p.secondary).wrap().padding(10.0))
        .child(receipt)
}

#[cfg(test)]
#[path = "help_section_tests.rs"]
mod tests;
