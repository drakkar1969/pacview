mod app;
mod window;
mod search_header;
mod search_tag;
mod filter_row;
mod pkgobject;
mod pkgproperty;

use gtk::{gio, glib};
use gtk::prelude::*;

use app::PacViewApplication;

const APP_ID: &str = "com.github.PacView";

fn main() -> glib::ExitCode {
    // Register and include resources
    gio::resources_register_include!("resources.gresource")
        .expect("Failed to register resources.");

    // Run app
    let app = PacViewApplication::new(APP_ID, &gio::ApplicationFlags::empty());

    app.run()
}
