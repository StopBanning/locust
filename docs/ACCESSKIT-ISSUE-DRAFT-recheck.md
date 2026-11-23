**Verdict (23 Nov 2026): still worth filing.** `setAccessibilitySelectedText:` is still unhandled on AccessKit's `main`
and in the newest `accesskit_macos` (0.27.2). No issue or PR covers it, and the proposed diff still fits the code.

# AccessKit: recheck of the 12 Nov draft issue. NOT POSTED.

Colin decides whether this goes up. Nothing has been filed, forked or commented on GitHub.

## What was checked, and how

- **AccessKit `main`:** `0977e030` (22 Nov 2026, "chore: Add an iOS example (#829)"), cloned from
  https://github.com/AccessKit/accesskit.
- **Newest release:** tag `accesskit_macos-v0.27.2` (`f3026222`, 25 Oct 2026); `adapters/macos/Cargo.toml` on `main` is
  still 0.27.2.
- `git diff accesskit_macos-v0.27.2 0977e030 -- adapters/macos/src/node.rs` is empty: the file is the same in the
  release and on `main`, as it was on 12 Nov (`dea4384`).

### 1. Is `setAccessibilitySelectedText:` handled now? No.

`adapters/macos/src/node.rs` on `main` has the getter `accessibilitySelectedText` (line 767) and the setters
`setAccessibilityValue:` (580) and `setAccessibilitySelectedTextRange:` (1031), but no `setAccessibilitySelectedText:`.
`isAccessibilitySelectorAllowed:` (from about line 1230) does not list it either.

### 2. Has anyone filed it or opened a PR? No.

GitHub search over `AccessKit/accesskit`, issues and pull requests separately, 23 Nov:

| Search | Issues | PRs |
| --- | --- | --- |
| `setAccessibilitySelectedText` | 0 | 0 |
| `AXSelectedText` | 0 | 0 |
| `ReplaceSelectedText` | 0 | 0 |
| `dictation` | 0 | 0 |
| `selected text macos` | 2, unrelated (#852 live regions, #824 AT-SPI combo box) | 3, unrelated (#756, #498, #289) |

### 3. Does the proposed diff still match the code? Yes.

- `resolve_with_context(|node, tree, context| ...)` is how every setter on `main` reaches the node, the tree and the
  context (for example `setAccessibilitySelectedTextRange:` at line 1031).
- `node.supports_text_ranges()` exists (line 367) and gates the other text selectors.
- `NodeRef::supports_action(action, &filter)` exists in `accesskit_consumer/src/node.rs` (line 691), and
  `isAccessibilitySelectorAllowed:` already uses it the same way for `setAccessibilityValue:`.
- `Action`, `ActionData` and `ActionRequest` are already imported at the top of `node.rs`; `filter` comes from
  `crate::filters`. `tree.state().locate_node(node.id())` is the call the other setters use.
- `Action::ReplaceSelectedText` is still in `accesskit` (`accesskit/src/lib.rs`), and AccessKit's own example text input
  (`example_common/src/widgets/text_input.rs`) supports it, so the example app is a ready repro.

New since 12 Nov, worth one line in the issue: the iOS adapter added in November lists `Action::ReplaceSelectedText`
among the actions it exposes (`adapters/ios/src/node.rs`), so macOS is now the Apple adapter that leaves it out.

**Not run here:** `cargo check -p accesskit_macos`. This check was on Linux, and the macOS adapter only builds on a Mac.
The 12 Nov check on the Mac (`cargo check -p accesskit_macos` and `cargo fmt --check` passing on `3577ace`) was against
the same `node.rs`. The Locust-side check (text inserted through accessibility reaching the message box) is still not
done.

---

## Revised issue title

macOS: `setAccessibilitySelectedText:` is not handled, so text inserted at the caret is silently dropped

## Revised issue body

On macOS, assistive apps insert text at the caret by setting `AXSelectedText` on the focused text field. Dictation and
text-expansion tools work this way. AccessKit's macOS adapter implements `setAccessibilitySelectedTextRange:` and
`setAccessibilityValue:`, but not `setAccessibilitySelectedText:`.

The call fails silently: the caller is told the attribute was set, and nothing happens. It affects any text field that
supports `Action::ReplaceSelectedText`, which is the action this setter should map to. AccessKit's doc for that action
says "Delete any selected text in the control's text value and insert the specified value in its place, like when
typing or pasting". The iOS adapter already exposes `ReplaceSelectedText`; macOS is the one Apple adapter without a way
in for it.

**To reproduce** (checked against `accesskit_macos` 0.27.2 and `main` at `0977e03`): run an app built on accesskit_macos
whose text field supports `ReplaceSelectedText` (the text input in `example_common` does). Focus the field. From another
process, set `AXSelectedText` to `"hello"`, for example from Swift:
`AXUIElementSetAttributeValue(field, kAXSelectedTextAttribute as CFString, "hello" as CFString)`. The call returns
success, but the field's text is unchanged and no `ReplaceSelectedText` action reaches the app.

**Expected:** the adapter forwards `ActionRequest { action: ReplaceSelectedText, data: Some(ActionData::Value(..)) }`,
as it already forwards `SetValue` for `setAccessibilityValue:` and `SetTextSelection` for
`setAccessibilitySelectedTextRange:`.

**Proposed fix:** a `setAccessibilitySelectedText:` method on the node, and an entry in
`isAccessibilitySelectorAllowed:`. Both are gated on `supports_text_ranges()` and on the node supporting
`ReplaceSelectedText`, so read-only fields and fields that don't support it are unchanged. Happy to open a PR.

```diff
--- a/adapters/macos/src/node.rs
+++ b/adapters/macos/src/node.rs
@@ after set_selected_text_range (setAccessibilitySelectedTextRange:) @@
+        #[method(setAccessibilitySelectedText:)]
+        fn set_selected_text(&self, string: &NSString) {
+            self.resolve_with_context(|node, tree, context| {
+                if node.supports_text_ranges()
+                    && node.supports_action(Action::ReplaceSelectedText, &filter)
+                {
+                    if let Some((target_node, target_tree)) = tree.state().locate_node(node.id()) {
+                        context.do_action(ActionRequest {
+                            action: Action::ReplaceSelectedText,
+                            target_tree,
+                            target_node,
+                            data: Some(ActionData::Value(string.to_string().into())),
+                        });
+                    }
+                }
+            });
+        }
+
@@ in isAccessibilitySelectorAllowed:, after the setAccessibilityValue: check @@
+                if selector == sel!(setAccessibilitySelectedText:) {
+                    return node.supports_text_ranges()
+                        && node.supports_action(Action::ReplaceSelectedText, &filter);
+                }
```

## What changed from the 12 Nov draft

- The versions checked are now `main` at `0977e03` (22 Nov) and 0.27.2; the file is unchanged since 12 Nov.
- The body names the example app as the repro and adds the iOS adapter line.
- The diff is the same code, with hunk notes saying where each part goes. It still needs rebasing onto `0977e03` on the
  Mac clone (`~/work/accesskit`, branch `macos-set-selected-text`) and the `cargo check` and `cargo fmt --check` run
  again there before any PR.
