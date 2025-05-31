use gtk::{glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use crate::pkg_object::PkgObject;

//------------------------------------------------------------------------------
// MODULE: SourceWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/source_window.ui")]
    pub struct SourceWindow {
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) text_view: TemplateChild<gtk::TextView>,
        #[template_child]
        pub(super) error_status: TemplateChild<adw::StatusPage>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for SourceWindow {
        const NAME: &'static str = "SourceWindow";
        type Type = super::SourceWindow;
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

    impl ObjectImpl for SourceWindow {}
    impl WidgetImpl for SourceWindow {}
    impl WindowImpl for SourceWindow {}
    impl AdwWindowImpl for SourceWindow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: SourceWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SourceWindow(ObjectSubclass<imp::SourceWindow>)
    @extends adw::Window, gtk::Window, gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl SourceWindow {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>, pkg: &PkgObject) -> Self {
        let obj: Self = glib::Object::builder()
            .property("transient-for", parent)
            .build();

        obj.set_title(Some(&format!("{}  \u{2022}  PKGBUILD", &pkg.name())));

        glib::spawn_future_local(clone!(
            #[weak] obj,
            #[weak] pkg,
            async move {
                let imp = obj.imp();

                let result = pkg.pkgbuild_future().await
                    .expect("Failed to complete tokio task");

                match result {
                    Ok(pkgbuild) => {
                        imp.text_view.buffer().set_text(&pkgbuild);

                        imp.stack.set_visible_child_name("text");
                    }
                    Err(error) => {
                        imp.error_status.set_description(Some(&error));

                        imp.stack.set_visible_child_name("error");
                    }
                }
            }
        ));

        obj
    }
}
