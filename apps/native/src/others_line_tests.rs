//! Electron's `the-others-line-describes-the-others.test.ts` (status.ts `deferredOthersSentence`).

use super::deferred_others_sentence;

const FROM_HERE: bool = true;
const FROM_VENDOR: bool = false;

#[test]
fn keeps_the_account_claim_only_when_it_holds_for_every_row() {
    assert_eq!(deferred_others_sentence(&[FROM_HERE; 3]), "3 others Locust can drive — each installs from here, then signs in.");
}

#[test]
fn says_vendor_when_none_of_them_install_from_here() {
    assert_eq!(deferred_others_sentence(&[FROM_VENDOR; 2]), "2 others Locust can drive — each installs from its own vendor.");
}

#[test]
fn claims_neither_when_the_list_is_mixed() {
    let sentence = deferred_others_sentence(&[FROM_HERE, FROM_HERE, FROM_HERE, FROM_VENDOR, FROM_VENDOR]);
    assert_eq!(sentence, "5 others Locust can drive — some install from here, some from their own vendor.");
    assert!(!sentence.contains("account"));
}

#[test]
fn counts_one_as_one() {
    assert!(deferred_others_sentence(&[FROM_VENDOR]).contains("1 other Locust"));
    assert!(!deferred_others_sentence(&[FROM_VENDOR]).contains("1 others"));
}
