use gtk::glib;
use adw::{prelude::*, subclass::prelude::*};

use crate::pkg_object::PkgObject;

//------------------------------------------------------------------------------
// MODULE: HashDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/hash_dialog.ui")]
    pub struct HashDialog {
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
    impl ObjectSubclass for HashDialog {
        const NAME: &'static str = "HashDialog";
        type Type = super::HashDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for HashDialog {}
    impl WidgetImpl for HashDialog {}
    impl AdwDialogImpl for HashDialog {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: HashDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct HashDialog(ObjectSubclass<imp::HashDialog>)
    @extends adw::Dialog, gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl HashDialog {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(pkg: &PkgObject) -> Self {
        let obj: Self = glib::Object::builder()
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

        let hashes = pkg.hashes();

        update_row(&imp.base64_row, hashes.base64_sig.as_deref());
        update_row(&imp.sha256_row, hashes.sha256sum.as_deref());
        update_row(&imp.md5_row, hashes.md5sum.as_deref());

        obj
    }
}
