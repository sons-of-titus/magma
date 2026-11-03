// The event bus's cascade bound, exercised end-to-end through the Janet
// surface — the only path where a handler can emit mid-dispatch (it
// reaches the editor through the thread-local scope and calls event/emit).

#[macro_use]
mod helpers;

use scripting;

#[test]
fn mutual_janet_event_emission_terminates() {
    janet_test!(ed, {
        scripting::eval(
            r#"(event/on "ts-cycle-a" (fn [_] (event/emit "ts-cycle-b" {})))
               (event/on "ts-cycle-b" (fn [_] (event/emit "ts-cycle-a" {})))"#,
        );
        scripting::eval("(event/emit \"ts-cycle-a\" {})");
        // The drain must stop on its own instead of cycling forever.
        ed.events.drain_and_dispatch();
    });
}

#[test]
fn a_janet_handler_chain_still_delivers_each_hop() {
    janet_test!(ed, {
        scripting::eval(
            r#"(var hops @[])
               (event/on "ts-chain-start" (fn [_] (event/emit "ts-chain-mid" {})))
               (event/on "ts-chain-mid" (fn [_] (array/push hops :mid) (event/emit "ts-chain-end" {})))
               (event/on "ts-chain-end" (fn [_] (array/push hops :end)))"#,
        );
        scripting::eval("(event/emit \"ts-chain-start\" {})");
        ed.events.drain_and_dispatch();
        let hops = scripting::eval_result("(string/join (map string hops) \",\")")
            .unwrap()
            .trim_matches('"')
            .to_string();
        assert_eq!(hops, "mid,end", "a two-hop handler chain still delivers");
    });
}
