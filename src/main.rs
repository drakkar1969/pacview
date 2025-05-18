mod app;
mod window;
mod search_bar;
mod search_tag;
mod filter_row;
mod package_view;
mod info_pane;
mod info_row;
mod history_list;
mod text_widget;
mod preferences_dialog;
mod stats_window;
mod backup_window;
mod log_window;
mod cache_window;
mod groups_window;
mod config_dialog;
mod pkg_data;
mod pkg_object;
mod stats_object;
mod backup_object;
mod log_object;
mod cache_object;
mod groups_object;
mod utils;
mod enum_traits;

use gtk::{gio, glib};
use gtk::prelude::*;

use app::PacViewApplication;

const APP_ID: &str = "com.github.PacView";

fn main() -> glib::ExitCode {
    // Register and include resources
    gio::resources_register_include!("resources.gresource")
        .expect("Failed to register resources");

    // Run app
    let app = PacViewApplication::new(APP_ID, gio::ApplicationFlags::default());

    app.run()
}
