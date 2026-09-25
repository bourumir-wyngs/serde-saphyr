fn main() {
    let _a = serde_saphyr::ser_options! { indent_step: 1 };
    let _b = serde_saphyr::ser_options! { indent_step: 64 };
    let _c = serde_saphyr::ser_options! { indent_step: 2, schema: serde_saphyr::specific! { quote_all: true } };
}
