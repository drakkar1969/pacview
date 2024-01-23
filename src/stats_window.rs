use std::collections::HashMap;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use titlecase::titlecase;

use crate::pkg_object::{PkgObject, PkgFlags};
use crate::stats_object::StatsObject;
use crate::utils::Utils;

//------------------------------------------------------------------------------
// MODULE: StatsWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/stats_window.ui")]
    pub struct StatsWindow {
        #[template_child]
        pub model: TemplateChild<gio::ListStore>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for StatsWindow {
        const NAME: &'static str = "StatsWindow";
        type Type = super::StatsWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            StatsObject::ensure_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for StatsWindow {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().setup_shortcuts();
        }
    }

    impl WidgetImpl for StatsWindow {}
    impl WindowImpl for StatsWindow {}
    impl AdwWindowImpl for StatsWindow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: StatsWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct StatsWindow(ObjectSubclass<imp::StatsWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl StatsWindow {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(parent: &gtk::Window, repo_names: &Vec<String>, pkg_model: &gio::ListStore) -> Self {
        let window: Self = glib::Object::builder()
            .property("transient-for", parent)
            .build();

        window.update_ui(repo_names, pkg_model);

        window
    }

    //-----------------------------------
    // Setup shortcuts
    //-----------------------------------
    fn setup_shortcuts(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();

        // Add close window shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(clone!(@weak self as window => @default-return true, move |_, _| {
                window.close();

                true
            })))
        ));

        // Add shortcut controller to window
        self.add_controller(controller);
    }

    //-----------------------------------
    // Update widgets
    //-----------------------------------
    fn update_ui(&self, repo_names: &Vec<String>, pkg_model: &gio::ListStore) {
        let imp = self.imp();

        // Create count, installed count, installed size maps
        let mut pcount_map: HashMap<String, i32> = HashMap::new();
        let mut icount_map: HashMap<String, i32> = HashMap::new();
        let mut isize_map: HashMap<String, i64> = HashMap::new();

        for repo in repo_names {
            pcount_map.insert(repo.to_string(), 0);
            icount_map.insert(repo.to_string(), 0);
            isize_map.insert(repo.to_string(), 0);
        }

        // Iterate through all packages
        pkg_model.iter::<PkgObject>().flatten()
            .for_each(|pkg| {
                // Increase repository total count
                pcount_map.entry(pkg.repository()).and_modify(|value| {
                    *value += 1;
                });

                // Increase repository installed count/size (if package installed)
                if pkg.flags().intersects(PkgFlags::INSTALLED) {
                    icount_map.entry(pkg.repository()).and_modify(|value| {
                        *value += 1;
                    });

                    isize_map.entry(pkg.repository()).and_modify(|value| {
                        *value += pkg.install_size();
                    });
                }
            });

        // Add item to stats column view for each repository
        for repo in repo_names {
            imp.model.append(&StatsObject::new(
                &titlecase(repo),
                &pcount_map[repo].to_string(),
                &icount_map[repo].to_string(),
                &Utils::size_to_string(isize_map[repo], 2)
            ));
        }

        // Add item with totals to stats column view
        imp.model.append(&StatsObject::new(
            "<b>Total</b>",
            &format!("<b>{}</b>", pcount_map.values().into_iter().sum::<i32>().to_string()),
            &format!("<b>{}</b>", icount_map.values().into_iter().sum::<i32>().to_string()),
            &format!("<b>{}</b>", &Utils::size_to_string(isize_map.values().into_iter().sum::<i64>(), 2))
        ));
    }
}
