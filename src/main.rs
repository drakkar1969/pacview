mod app;
mod window;
mod search_bar;
mod search_tag;
mod repo_item;
mod status_item;
mod package_view;
mod package_item;
mod tag_label;
mod info_pane;
mod info_details_tab;
mod info_files_tab;
mod hash_dialog;
mod info_row;
mod history_list;
mod text_widget;
mod preferences_dialog;
mod stats_window;
mod backup_dialog;
mod log_dialog;
mod cache_dialog;
mod groups_dialog;
mod config_dialog;
mod rootdir_dialog;
mod config_row;
mod source_window;
mod pkg_data;
mod pkg_object;
mod stats_object;
mod backup_object;
mod log_object;
mod cache_object;
mod groups_object;
mod tokio_manager;
mod utils;

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

    // Cancel all pending tasks
    app.connect_shutdown(|_| {
        tokio_manager::TokioManager::shutdown();
    });

    app.run()
}
