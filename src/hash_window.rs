use gtk::{glib, gdk};
use adw::{prelude::ActionRowExt, subclass::prelude::*};
use gtk::prelude::*;
use gdk::{Key, ModifierType};

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
        pub(super) base64_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) sha256_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) md5_row: TemplateChild<adw::ActionRow>,
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

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for HashWindow {}
    impl WidgetImpl for HashWindow {}
    impl WindowImpl for HashWindow {}
    impl AdwWindowImpl for HashWindow {}

    impl HashWindow {
        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Close window binding
            klass.add_binding_action(Key::Escape, ModifierType::NO_MODIFIER_MASK, "window.close");
        }
    }
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
    pub fn new(parent: &impl IsA<gtk::Window>, pkg: &PkgObject) -> Self {
        let obj: Self = glib::Object::builder()
            .property("transient-for", parent)
            .property("title", format!("{}  \u{2022}  Hashes", &pkg.name()))
            .build();

        let imp = obj.imp();

        // Helper closure
        let update_row = |row: &adw::ActionRow, hash: Option<&str> | {
            if let Some(hash) = hash {
                row.set_visible(true);
                row.set_subtitle(hash);
            } else {
                row.set_visible(false);
            }
        };

        update_row(&imp.base64_row, pkg.base64_sig().as_deref());
        update_row(&imp.sha256_row, pkg.sha256sum().as_deref());
        update_row(&imp.md5_row, pkg.md5sum().as_deref());

        obj
    }
}
