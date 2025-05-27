use gtk::{glib, gdk};
use adw::{prelude::ActionRowExt, subclass::prelude::*};
use gtk::prelude::*;

use crate::pkg_data::PkgValidation;
use crate::pkg_object::PkgObject;

//------------------------------------------------------------------------------
// MODULE: HashWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/hash_window.ui")]
    pub struct HashWindow {
        #[template_child]
        pub(super) md5_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) sha256_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) base64_row: TemplateChild<adw::ActionRow>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for HashWindow {
        const NAME: &'static str = "HashWindow";
        type Type = super::HashWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            //---------------------------------------
            // Add class key bindings
            //---------------------------------------
            // Close window binding
            klass.add_binding_action(gdk::Key::Escape, gdk::ModifierType::NO_MODIFIER_MASK, "window.close");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for HashWindow {}
    impl WidgetImpl for HashWindow {}
    impl WindowImpl for HashWindow {}
    impl AdwWindowImpl for HashWindow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: HashWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct HashWindow(ObjectSubclass<imp::HashWindow>)
    @extends adw::Window, gtk::Window, gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl HashWindow {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, pkg: &PkgObject) {
        let imp = self.imp();

        self.set_title(Some(&pkg.name()));

        let validation = pkg.validation();
        let hashes = pkg.hashes();

        if validation.intersects(PkgValidation::MD5SUM) {
            imp.md5_row.set_visible(true);
            imp.md5_row.set_subtitle(hashes.md5sum().unwrap_or("(None)"));
        } else {
            imp.md5_row.set_visible(false);
        }

        if validation.intersects(PkgValidation::SHA256SUM) {
            imp.sha256_row.set_visible(true);
            imp.sha256_row.set_subtitle(hashes.sha256sum().unwrap_or("(None)"));
        } else {
            imp.sha256_row.set_visible(false);
        }

        if validation.intersects(PkgValidation::SIGNATURE) {
            imp.base64_row.set_visible(true);
            imp.base64_row.set_subtitle(hashes.base64_sig().unwrap_or("(None)"));
        } else {
            imp.base64_row.set_visible(false);
        }

        self.present();
    }
}
