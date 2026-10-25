#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icons/beamwright.ico");
    if let Err(e) = res.compile() {
        eprintln!("winres: {e}");
    }
}

#[cfg(not(windows))]
fn main() {}
