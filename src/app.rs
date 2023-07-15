use std::cell::Cell;

use gtk::{gio, glib};
use gtk::prelude::*;
use adw::subclass::prelude::*;
use gtk::gdk;
use glib::clone;

use crate::window::PacViewWindow;

//------------------------------------------------------------------------------
// GLOBAL VARIABLES
//------------------------------------------------------------------------------
thread_local! {
    pub static LINK_RGBA: Cell<gdk::RGBA> = Cell::new(gdk::RGBA::BLUE);
}

//------------------------------------------------------------------------------
// MODULE: PacViewApplication
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default)]
    pub struct PacViewApplication {}

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PacViewApplication {
        const NAME: &'static str = "PacViewApplication";
        type Type = super::PacViewApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for PacViewApplication {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().setup_actions();
        }
    }

    impl ApplicationImpl for PacViewApplication {
        //-----------------------------------
        // Activate handler
        //-----------------------------------
        fn activate(&self) {
            let application = self.obj();

            self.setup_styles();

            // Show main window
            let window = if let Some(window) = application.active_window() {
                window
            } else {
                let window = PacViewWindow::new(&*application);
                window.upcast()
            };

            window.present();
        }
    }

    impl GtkApplicationImpl for PacViewApplication {}
    impl AdwApplicationImpl for PacViewApplication {}

    impl PacViewApplication {
        //-----------------------------------
        // Setup styles
        //-----------------------------------
        fn setup_styles(&self) {
            // Get link color
            LINK_RGBA.with(|rgba| {
                let link_btn = gtk::LinkButton::new("www.gtk.org");

                rgba.replace(link_btn.color());
            });

            // Get style manager
            let style_manager = adw::StyleManager::default();

            // Get icon theme for default display
            let display = gdk::Display::default().unwrap();

            let icon_theme = gtk::IconTheme::for_display(&display);

            // Set icon resource paths
            let icons_light_path = "/com/github/PacView/icons-light/";
            let icons_dark_path = "/com/github/PacView/icons-dark/";

            if style_manager.is_dark() {
                icon_theme.add_resource_path(icons_dark_path);
            } else {
                icon_theme.add_resource_path(icons_light_path);
            }

            // Connect style manager dark property notify signal
            style_manager.connect_dark_notify(clone!(@weak style_manager => move |style| {
                // Update link color when color scheme changes
                LINK_RGBA.with(|rgba| {
                    let link_btn = gtk::LinkButton::new("www.gtk.org");

                    let btn_style = adw::StyleManager::for_display(&link_btn.display());

                    if style.is_dark() {
                        btn_style.set_color_scheme(adw::ColorScheme::ForceDark);
                    } else {
                        btn_style.set_color_scheme(adw::ColorScheme::ForceLight);
                    }
    
                    rgba.replace(link_btn.color());
                });

                // Update icon resource paths when color scheme changes
                let resource_paths = icon_theme.resource_path();

                let mut icon_paths: Vec<String> = resource_paths.iter()
                    .filter_map(|s| {
                        if s.contains(icons_light_path) || s.contains(icons_dark_path) {
                            None
                        } else {
                            Some(s.to_string())
                        }
                    })
                    .collect();

                if style_manager.is_dark() {
                    icon_paths.push(icons_dark_path.to_string());
                } else {
                    icon_paths.push(icons_light_path.to_string());
                }

                icon_theme.set_resource_path(&icon_paths.iter().map(|s| s.as_str()).collect::<Vec<&str>>());
            }));
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PacViewApplication
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PacViewApplication(ObjectSubclass<imp::PacViewApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl PacViewApplication {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(application_id: &str, flags: &gio::ApplicationFlags) -> Self {
        glib::Object::builder()
            .property("application-id", application_id)
            .property("flags", flags)
            .build()
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        let quit_action = gio::ActionEntry::builder("quit-app")
            .activate(move |app: &Self, _, _| app.quit())
            .build();
        let about_action = gio::ActionEntry::builder("show-about")
            .activate(move |app: &Self, _, _| app.show_about())
            .build();
        self.add_action_entries([quit_action, about_action]);

        self.set_accels_for_action("app.quit-app", &["<ctrl>Q"]);
        self.set_accels_for_action("app.show-about", &["F1"]);

        self.set_accels_for_action("win.show-sidebar", &["<ctrl>B"]);
        self.set_accels_for_action("win.show-infopane", &["<ctrl>I"]);
        self.set_accels_for_action("win.show-preferences", &["<ctrl>comma"]);

        self.set_accels_for_action("search.start", &["<ctrl>F"]);
        self.set_accels_for_action("search.stop", &["Escape"]);

        self.set_accels_for_action("view.refresh", &["F5"]);
        self.set_accels_for_action("view.show-stats", &["<alt>S"]);
        self.set_accels_for_action("view.copy-list", &["<alt>L"]);

        self.set_accels_for_action("info.previous", &["<alt>Left"]);
        self.set_accels_for_action("info.next", &["<alt>Right"]);
        self.set_accels_for_action("info.show-details", &["<alt>Return", "<alt>KP_Enter"]);
    }

    //-----------------------------------
    // Show about window
    //-----------------------------------
    fn show_about(&self) {
        let window = self.active_window().unwrap();

        let about_window = adw::AboutWindow::builder()
            .transient_for(&window)
            .application_name("PacView")
            .application_icon("software-properties")
            .developer_name("draKKar1969")
            .version("2.0.12")
            .website("https://github.com/drakkar1969/pacview")
            .developers(vec!["draKKar1969"])
            .designers(vec!["draKKar1969"])
            .copyright("© 2023 draKKar1969")
            .license_type(gtk::License::Gpl30)
            .build();

        about_window.present();
    }
}
