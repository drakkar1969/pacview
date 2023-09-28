use gtk::{gio, glib, gdk};
use gtk::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;

use crate::window::PacViewWindow;

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
            style_manager.connect_dark_notify(clone!(@weak style_manager => move |_| {
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
            .version("2.1.0")
            .website("https://github.com/drakkar1969/pacview")
            .developers(vec!["draKKar1969"])
            .designers(vec!["draKKar1969"])
            .copyright("© 2023 draKKar1969")
            .license_type(gtk::License::Gpl30)
            .build();

        about_window.present();
    }
}
