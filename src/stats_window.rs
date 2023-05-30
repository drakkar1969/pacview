use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;

use titlecase;

use crate::pkg_object::{PkgObject, PkgFlags};
use crate::stats_object::StatsObject;

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
            StatsObject::static_type();

            klass.bind_template();
            klass.bind_template_callbacks();
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
        }
    }

    impl WidgetImpl for StatsWindow {}
    impl WindowImpl for StatsWindow {}
    impl AdwWindowImpl for StatsWindow {}

    //-----------------------------------
    // Key press signal handler
    //-----------------------------------
    #[gtk::template_callbacks]
    impl StatsWindow {
        #[template_callback]
        fn on_key_pressed(&self, key: u32, _: u32, state: gdk::ModifierType) -> bool {
            if key == 65307 && state.is_empty() {
                self.obj().close();
            }

            true
        }
    }
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION: StatsWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct StatsWindow(ObjectSubclass<imp::StatsWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl StatsWindow {
    //-----------------------------------
    // Public new function
    //-----------------------------------
    pub fn new(repo_names: &Vec<String>, pkg_list: &Vec<PkgObject>) -> Self {
        let window: Self = glib::Object::builder().build();

        let imp = window.imp();

        let mut total_pcount = 0;
        let mut total_icount = 0;
        let mut total_isize = 0;

        // For each repository
        for repo in repo_names {
            // Find packages in repository and get count
            let repo_list: Vec<&PkgObject> = pkg_list.into_iter()
                .filter(|&pkg| pkg.repository() == *repo)
                .collect();

            let pcount = repo_list.len();
            total_pcount += pcount;

            // Find installed packages and get count + total size
            let installed_list: Vec<&PkgObject> = repo_list.into_iter()
                .filter(|&pkg| pkg.flags().intersects(PkgFlags::INSTALLED))
                .collect();

            let icount = installed_list.len();
            total_icount += icount;

            let isize: i64 = installed_list.iter().map(|pkg| pkg.install_size()).sum();
            total_isize += isize;

            // Add repository item to stats column view
            imp.model.append(&StatsObject::new(
                &titlecase::titlecase(repo),
                &pcount.to_string(),
                &icount.to_string(),
                &PkgObject::size_to_string(isize, 2)
            ));
        }

        // Add item with totals to stats column view
        imp.model.append(&StatsObject::new(
            "<b>Total</b>",
            &format!("<b>{}</b>", total_pcount.to_string()),
            &format!("<b>{}</b>", total_icount.to_string()),
            &format!("<b>{}</b>", &PkgObject::size_to_string(total_isize, 2))
        ));

        window
    }
}
