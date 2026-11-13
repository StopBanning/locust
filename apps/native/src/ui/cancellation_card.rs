//! A stopped run's receipt (Electron `CancellationCard.tsx`): what reported finishing, what was open when it stopped,
//! what of the plan never started, and the one thing a stop cannot promise. Standing, not amber: it reports a run that
//! already stopped and asks for nothing. Only a Stop pressed in this window reads "You stopped this run".

use super::*;
use locust_core::cancellation::{self, Summary};

pub(super) fn card(summary: &Summary, by_person: bool, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let opencode = summary.tools_reported_when_done;
    let activity = !summary.settled.is_empty() || !summary.interrupted.is_empty();
    let title = if by_person { "You stopped this run" } else { "This run was stopped" };
    let mut out = col()
        .w_full()
        .gap(8.0)
        .padding(12.0)
        .radius(8.0)
        .border(1.0, l.c("border-card"))
        .key("stopped-card")
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(text(title).font(l.ty.control).color(l.c("text-primary")).grow(1.0))
                .child(text("Stopped").font(type_::LABEL).color(l.c("amber-text"))),
        );
    let line = |label: &str, body: El| row().w_full().gap(10.0).child(text(label.to_owned()).font(type_::LABEL).color(l.c("text-muted")).w(84.0)).child(body.grow(1.0));
    let list = |names: &[String], colour| {
        names.iter().fold(col().gap(2.0), |c, n| c.child(text(n.clone()).font(type_::LABEL.mono()).color(colour).wrap_anywhere()))
    };
    if cancellation::stopped_before_any_tool(summary) && !opencode {
        out = out.child(text(cancellation::BEFORE_ANY_TOOL).font(l.ty.body).color(l.c("text-secondary")).wrap());
    } else {
        if let Some(plan) = cancellation::plan_line(summary) {
            out = out.child(line("Plan", text(plan).font(l.ty.body).color(l.c("text-secondary")).wrap()));
        }
        let finished = if summary.settled.is_empty() {
            text(cancellation::NOTHING_FINISHED).font(l.ty.body).color(l.c("text-secondary"))
        } else {
            list(&summary.settled, l.c("text-primary"))
        };
        out = out.child(line("Finished", finished));
        let cut = if summary.interrupted.is_empty() {
            text(if opencode { cancellation::UNREPORTED_COMMAND } else { cancellation::NONE_OPEN }).font(l.ty.body).color(if opencode { l.c("amber-text") } else { l.c("text-secondary") }).wrap()
        } else {
            let mut c = list(&summary.interrupted, l.c("amber-text")).child(text(cancellation::UNKNOWN_EFFECT).font(l.ty.body).color(l.c("amber-text")).wrap());
            if opencode {
                c = c.child(text(cancellation::UNREPORTED_COMMAND).font(l.ty.body).color(l.c("amber-text")).wrap());
            }
            c
        };
        out = out.child(line("Cut off", cut));
        if summary.plan.is_none() && summary.never_started > 0 {
            let n = summary.never_started;
            out = out.child(line("Never started", text(format!("{n} planned step{}", if n == 1 { "" } else { "s" })).font(l.ty.body).color(l.c("text-secondary"))));
        }
    }
    if activity {
        out = out.child(text(cancellation::IN_THE_RECORD).font(type_::LABEL).color(l.c("text-muted")).wrap());
        if !opencode {
            out = out.child(text(cancellation::MAY_STILL_FINISH).font(type_::LABEL).color(l.c("amber-text")).wrap());
        }
    }
    out
}
