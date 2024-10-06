use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;

use titlecase::titlecase;

use crate::pkg_object::{PkgObject, PkgFlags};
use crate::stats_object::StatsObject;
use crate::utils::size_to_string;

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
        pub(super) model: TemplateChild<gio::ListStore>,
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

            klass.add_shortcut(&gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string("Escape"),
                Some(gtk::CallbackAction::new(|widget, _| {
                    let window = widget
                        .downcast_ref::<crate::stats_window::StatsWindow>()
                        .expect("Could not downcast to 'StatsWindow'");

                    window.close();

                    glib::Propagation::Proceed
                }))
            ))
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for StatsWindow {}
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
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    //-----------------------------------
    // Show window
    //-----------------------------------
    pub fn show(&self, repo_names: &[String], pkg_snapshot: &[PkgObject]) {
        let imp = self.imp();

        // Iterate repos
        let (tot_pcount, tot_icount, tot_isize) = repo_names.iter()
            .fold((0, 0, 0), |(tot_pcount, tot_icount, tot_isize), repo| {
                // Iterate packages per repo
                let (pcount, icount, isize) = pkg_snapshot.iter()
                    .filter(|pkg| pkg.repository() == *repo)
                    .fold((0, 0, 0), |(mut pcount, mut icount, mut isize), pkg| {
                        pcount += 1;

                        if pkg.flags().intersects(PkgFlags::INSTALLED) {
                            icount += 1;
                            isize += pkg.install_size()
                        }

                        (pcount, icount, isize)
                    });

                // Add repo item to stats column view
                let repo = if repo == "aur" { repo.to_uppercase() } else { titlecase(repo) };

                imp.model.append(&StatsObject::new(
                    &repo,
                    &pcount.to_string(),
                    &icount.to_string(),
                    &size_to_string(isize, 2)
                ));

                (tot_pcount + pcount, tot_icount + icount, tot_isize + isize)
            });

        // Add item with totals to stats column view
        imp.model.append(&StatsObject::new(
            "<b>Total</b>",
            &format!("<b>{}</b>", tot_pcount),
            &format!("<b>{}</b>", tot_icount),
            &format!("<b>{}</b>", &size_to_string(tot_isize, 2))
        ));

        self.present();
    }
}
