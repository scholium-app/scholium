// Test-only fault amplifier: remove TeX's shell restriction so OS isolation is tested.
use std::os::unix::process::CommandExt;
fn main() {
    let error = std::process::Command::new("/usr/bin/luahbtex")
        .arg0("/usr/bin/lualatex")
        .arg("--shell-escape")
        .args(std::env::args_os().skip(1))
        .exec();
    eprintln!("{error}");
    std::process::exit(127);
}
