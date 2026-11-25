
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref()
        == Ok("windows")
    {
        let mut res = winres::WindowsResource::new();

        res.set_icon("assets/DualLink-icon.ico");
        res.set(
            "FileDescription",
            "DualLink Controller Studio",
        );
        res.set(
            "ProductName",
            "DualLink Controller Studio",
        );

        res.compile()
            .expect("Failed to compile Windows resources");
    }
}
