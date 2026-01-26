fn main() {
    let _a = serde_saphyr::ser_options! { indent_step: 1 };
    let _b = serde_saphyr::ser_options! { indent_step: 65535 };
    let _c = serde_saphyr::serializer_options! { indent_step: 2, quote_all: true };
}
