use serde_saphyr::{FoldStr, FoldString, LitStr, LitString};

#[test]
fn block_string_wrappers_support_basic_conversions_and_comparisons() {
    let lit_borrowed: LitStr<'_> = "hello".into();
    let fold_borrowed: FoldStr<'_> = "hello".into();

    assert_eq!(&*lit_borrowed, "hello");
    assert_eq!(&*fold_borrowed, "hello");

    let lit_owned: LitString = "hello".to_string().into();
    let fold_owned: FoldString = "hello".to_string().into();

    // Deref to &str
    assert_eq!(&*lit_owned, "hello");
    assert_eq!(&*fold_owned, "hello");

    // Compare to each other
    assert_eq!(lit_owned, fold_owned);
    assert_eq!(fold_owned, lit_owned);

    // Compare to String and str
    assert!(lit_owned == "hello");
    assert!(fold_owned == "hello");
}

#[test]
fn block_string_wrappers_can_be_unwrapped() {
    let lit: LitString = "x".to_string().into();
    let fold: FoldString = "y".to_string().into();
    assert_eq!(lit.into_inner(), "x");
    assert_eq!(fold.into_inner(), "y");
}
