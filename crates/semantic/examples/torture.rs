//! Parse a battery of modern-Rust constructs with a bundled grammar and
//! report which ones produce ERROR/MISSING nodes.
//!
//! Usage: cargo run -p semantic --example torture_check -- <lang>

use semantic::ts_registry;
use std::path::Path;

const SNIPPETS: &[(&str, &str)] = &[
    ("let-else", "fn f() { let Some(x) = opt else { return; }; }"),
    ("if-let-chain", "fn f(a: Option<i32>) { if let Some(x) = a && x > 0 { println!(\"{x}\"); } }"),
    ("c-string-literal", "fn f() { let p: &CStr = c\"path\"; let r = cr\"raw\"; }"),
    ("raw-pointer-refs", "fn f(p: *const u8) { let r = &raw const *p; let m = &raw mut *p; }"),
    ("unsafe-extern", "unsafe extern \"C-unwind\" { fn g() -> i32; }"),
    ("unsafe-extern-block-type", "unsafe extern \"C\"-block {}"),
    ("async-fn-in-trait", "trait T { async fn f(&self) -> i32; }"),
    ("return-position-impl-trait", "trait T { fn f() -> impl Trait + Send; }"),
    ("async-trait-rpitit", "trait T { async fn f() -> impl Into<u8>; }"),
    ("gats", "trait LendingIterator { type Item<'a> where Self: 'a; fn next(&mut self) -> Option<Self::Item<'_>>; }"),
    ("gen-block", "fn f() { gen { yield 1; } }"),
    ("labeled-block", "fn f() { 'blk: { break 'blk; } }"),
    ("labeled-while-let", "fn f() { 'a: while let Some(x) = it { break 'a; } }"),
    ("nested-macros", "fn f() { println!(\"{}\", format!(\"{:?}\", vec![1, 2])); }"),
    ("macro-attr-macro", "#[cfg(feature = \"x\")]\n#[derive(Debug, Clone)]\nstruct S { f: u8 }"),
    ("attr-with-eq-string", "#[doc = \"hello\"]\nfn f() {}"),
    ("impl-assoc-const", "struct S;\nimpl S { const N: usize = 4; }"),
    ("trait-object-paren", "fn f(b: Box<dyn FnOnce() -> Box<dyn Iterator<Item = u8>>>) {}"),
    ("lifetime-bounds", "fn f<'a, 'b: 'a>(x: &'a str) -> &'b str where 'a: 'b { x }"),
    ("const-generics", "struct A<const N: usize = 3>;\nfn f() -> A<4> { A }"),
    ("const-generic-default-expr", "struct B<const N: usize = {1 + 2}>;"),
    ("closure-capture-impl", "fn f() { let c = move || async { 1 }; }"),
    ("dyn-star", "fn f(d: dyn* Send) {}"),
    ("try-block", "fn f() { let x = try { 42 }; }"),
    ("raw-string-hashes", "fn f() { let s = r##\"raw \"quoted\"\"##; }"),
    ("byte-string", "fn f() { let b: &[u8] = b\"bytes\"; let rb = br\"raw#\"; }"),
    ("negative-impl", "impl !Send for S {}"),
    ("auto-trait-impl-fn", "auto trait Send {}"),
    ("where-clause-const", "fn f<const N: usize>() -> [u8; N] where [(); N]: { [0; N] }"),
    ("op-assign-traits", "fn f() { let mut x = 1; x += 1; }"),
    ("await-in-match", "async fn f(fut: impl Future<Output = i32>) { match fut.await { x => {} } }"),
    ("string-unicode", "fn f() { let s = \"emoji 🦀 and ünïcödé\"; }"),
    ("deep-generics", "fn f(m: std::collections::HashMap<String, Vec<Option<Box<dyn std::error::Error>>>>) {}"),
    ("shift-assign", "fn f() { let mut x = 1u64; x <<= 2; x >>= 1; }"),
    ("question-in-closure", "fn f() -> Result<(), E> { let g = || -> Result<(), E> { Ok(a()?) }; Ok(()) }"),
    ("struct-update-fn-call", "fn f() { let s = S { a: 1, ..Default::default() }; }"),
    ("anon-lifetime-in-impl", "struct S;\nimpl S { fn f(&self) -> &str { \"x\" } }"),
    ("tuple-struct-pattern", "fn f() { let Point(x, y) = p; let (a, b) = (1, 2); }"),
    ("range-pattern", "fn f(x: i32) { match x { 1..=5 => {}, 6.. => {}, _ => {} } }"),
    ("or-pattern", "fn f(x: Option<i32>) { match x { Some(1 | 2) => {}, None => {} } }"),
    ("slice-pattern", "fn f(s: &[u8]) { match s { [a, rest @ ..] => {}, _ => {} } }"),
    ("generic-const-where", "trait T<const N: usize> {}"),
    ("qualified-path", "fn f() { <S as Trait>::CONST; let x: <S as Trait>::Assoc = todo!(); }"),
    ("never-type", "fn f() -> ! { panic!() }"),
    ("extern-type", "extern type Opaque;"),
    ("dyn-trait-upcasting", "trait A {}\ntrait B: A {}\nfn f(b: &dyn B) -> &dyn A { b }"),
    ("fn-ptr-qual", "fn f(g: unsafe extern \"C\" fn(u8) -> u8) {}"),
];

fn main() {
    let mut args = std::env::args().skip(1);
    let lang = args.next().unwrap_or_else(|| "rust".to_string());
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let dylib = plugins
        .join(&lang)
        .join(format!("grammars/tree-sitter-{lang}.dylib"));
    let language =
        ts_registry::load_grammar_from_lib(dylib.to_str().unwrap(), &lang, None).expect("loads");
    ts_registry::register_language(&lang, language);

    let mut bad = 0;
    for (name, src) in SNIPPETS {
        let errors = ts_registry::parse_text(&lang, src).unwrap_or_default();
        if !errors.is_empty() {
            bad += 1;
            println!("BROKEN  {name}: {} error node(s) — {}", errors.len(), src);
        }
    }
    println!("{}/{} snippets parse clean", SNIPPETS.len() - bad, SNIPPETS.len());
}
