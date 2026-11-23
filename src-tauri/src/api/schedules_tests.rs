use crate::text::split_recipients;

#[test]
fn splits_notification_recipients() {
    assert_eq!(
        split_recipients(" a@x.com, b@x.com; c@x.com "),
        vec!["a@x.com", "b@x.com", "c@x.com"]
    );
    assert!(split_recipients("  ").is_empty());
}
