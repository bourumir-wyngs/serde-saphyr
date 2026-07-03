fn main() {
    let _ = serde_saphyr::options! {
        require_indent: serde_saphyr::RequireIndent::Divisible(0),
    };
}
