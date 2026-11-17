// Carries apps/desktop/src/shared/a-torn-tag-keeps-the-letters-before-it.test.ts (Electron 4ce2f37a).
use crate::protocol_tags::unwrap_protocol_tags;

#[test]
fn a_tag_cut_off_at_the_end_of_a_stream_is_removed_and_the_letters_before_it_kept() {
    assert_eq!(unwrap_protocol_tags("Yes, the tests pass<locust-sh"), "Yes, the tests pass");
    assert_eq!(unwrap_protocol_tags("Done: 3 files<locu"), "Done: 3 files");
    assert_eq!(unwrap_protocol_tags("Yes, the tests pass <locust-sh"), "Yes, the tests pass");
    assert_eq!(unwrap_protocol_tags("It works.\n</locu"), "It works.");
    assert_eq!(unwrap_protocol_tags("Tests pass"), "Tests pass");
}
