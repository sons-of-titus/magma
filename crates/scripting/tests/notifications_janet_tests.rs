#[macro_use]
mod helpers;
use scripting;

// The notification center holds failures a user must not miss — the
// transient status message evaporates, these stay until dismissed.
#[test]
fn notifications_post_list_and_dismiss() {
    janet_test!(ed, {
        let id = scripting::eval_result(
            r#"(notify/post "error" "Language server not found" "rust-analyzer missing")"#,
        )
        .unwrap_or_default();
        assert!(
            id.parse::<i64>().is_ok(),
            "posting returns the notification id: {id}"
        );

        let listed = scripting::eval_result(
            r#"(let [n (first (notifications))]
                 (string (get n :kind) "|" (get n :title) "|" (get n :body)))"#,
        )
        .unwrap_or_default();
        assert_eq!(
            listed,
            r#""error|Language server not found|rust-analyzer missing""#,
            "the card carries kind, title and body"
        );

        assert_eq!(
            scripting::eval_result(&format!("(notify/dismiss {id})")).unwrap_or_default(),
            ":ok"
        );
        let count =
            scripting::eval_result(r#"(length (notifications))"#).unwrap_or_default();
        assert_eq!(count, "0");
        assert_eq!(
            scripting::eval_result(&format!("(notify/dismiss {id})")).unwrap_or_default(),
            "nil",
            "dismissing twice is nil"
        );
    });
}

// The formatter's missing-binary report posts a card, not just a whisper.
#[test]
fn formatter_failure_posts_a_notification() {
    janet_test!(ed, {
        scripting::eval(
            r#"(formatter/register "zznotify" {:command "zz-no-such-formatter" :args @[]})"#,
        );
        scripting::eval_result(r#"(formatter/format "zznotify" "text")"#).unwrap();
        let listed = scripting::eval_result(r#"(length (notifications))"#)
            .unwrap_or_default();
        assert_eq!(listed, "1", "the missing formatter is a notification");
    });
}
