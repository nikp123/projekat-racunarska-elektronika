fn main() {
    let gpiod = pkg_config::probe_library("libgpiod").unwrap();

    cc::Build::new()
        .file("src/gpio.c")
        .includes(gpiod.include_paths[0].to_str())
        .flags(["-lgpiod"])
        .define("RUST", None)
        .compile("gpio");
    slint_build::compile("ui/main.slint").expect("Slint build failed");
}
