use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use adw::prelude::AdwDialogExt;

use titlecase::titlecase;

use crate::pkg_object::{PkgObject, PkgFlags};
use crate::stats_object::StatsObject;
use crate::utils::Utils;

//------------------------------------------------------------------------------
// MODULE: StatsDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/stats_dialog.ui")]
    pub struct StatsDialog {
        #[template_child]
        pub model: TemplateChild<gio::ListStore>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for StatsDialog {
        const NAME: &'static str = "StatsDialog";
        type Type = super::StatsDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            StatsObject::ensure_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for StatsDialog {}
    impl WidgetImpl for StatsDialog {}
    impl AdwDialogImpl for StatsDialog {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: StatsDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct StatsDialog(ObjectSubclass<imp::StatsDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl StatsDialog {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Update widgets
    //-----------------------------------
    fn update_ui(&self, repo_names: &[String], pkg_snapshot: &[PkgObject]) {
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
                    &Utils::size_to_string(isize, 2)
                ));

                (tot_pcount + pcount, tot_icount + icount, tot_isize + isize)
            });

        // Add item with totals to stats column view
        imp.model.append(&StatsObject::new(
            "<b>Total</b>",
            &format!("<b>{}</b>", tot_pcount),
            &format!("<b>{}</b>", tot_icount),
            &format!("<b>{}</b>", &Utils::size_to_string(tot_isize, 2))
        ));
    }

    //-----------------------------------
    // Public show function
    //-----------------------------------
    pub fn show(&self, parent: &impl IsA<gtk::Widget>, repo_names: &[String], pkg_snapshot: &[PkgObject]) {
        self.update_ui(repo_names, pkg_snapshot);

        self.present(parent);
    }
}

impl Default for StatsDialog {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
