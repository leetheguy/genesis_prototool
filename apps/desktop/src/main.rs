//! `genesis-desktop [--serve URL] [APP]`: a native window on a `serve` page. See the library.

fn main() {
    let args = std::env::args().skip(1).collect();
    let url = match genesis_desktop::window_url(args, std::env::var("GENESIS_SERVE").ok()) {
        Ok(url) => url,
        Err(why) => {
            eprintln!("{why}");
            std::process::exit(2);
        }
    };
    let built = tauri::Builder::default()
        .setup(move |app| {
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::External(url.clone()))
                .title("Genesis")
                .inner_size(1100.0, 760.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(e) = built {
        eprintln!("the window couldn't open: {e}");
        std::process::exit(1);
    }
}
