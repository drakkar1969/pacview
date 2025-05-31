use std::cell::OnceCell;
use std::marker::PhantomData;

use gtk::{glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use sourceview5;
use sourceview5::prelude::*;

use crate::pkg_object::PkgObject;

//------------------------------------------------------------------------------
// MODULE: SourceWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::SourceWindow)]
    #[template(resource = "/com/github/PacView/ui/source_window.ui")]
    pub struct SourceWindow {
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) refresh_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) source_view: TemplateChild<sourceview5::View>,
        #[template_child]
        pub(super) error_status: TemplateChild<adw::StatusPage>,

        #[property(get = Self::buffer)]
        buffer: PhantomData<sourceview5::Buffer>,
        #[property(get, set)]
        pkg: OnceCell<PkgObject>,
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

            // Refresh binding
            klass.add_binding(gdk::Key::F5, gdk::ModifierType::NO_MODIFIER_MASK, |window| {
                window.imp().refresh_button.emit_clicked();

                glib::Propagation::Stop
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SourceWindow {
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().setup_signals();
        }
    }

    impl WidgetImpl for SourceWindow {}
    impl WindowImpl for SourceWindow {}
    impl AdwWindowImpl for SourceWindow {}

    impl SourceWindow {
        //---------------------------------------
        // Property getter
        //---------------------------------------
        fn buffer(&self) -> sourceview5::Buffer {
            self.source_view.buffer()
                .downcast::<sourceview5::Buffer>()
                .expect("Failed to downcast to 'SourceBuffer'")
        }
    }
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
            .property("title", format!("{}  \u{2022}  PKGBUILD", &pkg.name()))
            .property("pkg", pkg)
            .build();

        // Set syntax highlighting language
        let buffer = obj.buffer();

        if let Some(language) = sourceview5::LanguageManager::default().language("pkgbuild") {
            buffer.set_language(Some(&language));
        }

        // Set style scheme
        let style_manager = adw::StyleManager::for_display(&gtk::prelude::WidgetExt::display(&obj));

        let style = if style_manager.is_dark() {
            "one-dark"
        } else {
            "one"
        };

        let scheme_manager = sourceview5::StyleSchemeManager::default();

        buffer.set_style_scheme(scheme_manager.scheme(style).as_ref());

        // Download PKGBUILD
        obj.download_pkgbuild();

        obj
    }

    //---------------------------------------
    // Download PKGBUILD function
    //---------------------------------------
    fn download_pkgbuild(&self) {
        let imp = self.imp();

        imp.stack.set_visible_child_name("loading");

        glib::spawn_future_local(clone!(
            #[weak(rename_to = window)] self,
            #[weak] imp,
            async move {
                let result = window.pkg().pkgbuild_future().await
                    .expect("Failed to complete tokio task");

                match result {
                    Ok(pkgbuild) => {
                        let buffer = window.buffer();

                        buffer.set_text(&pkgbuild);

                        // Position cursor at start
                        buffer.place_cursor(&window.buffer().iter_at_offset(0));

                        imp.stack.set_visible_child_name("text");
                    }
                    Err(error) => {
                        imp.error_status.set_description(Some(&error));

                        imp.stack.set_visible_child_name("error");
                    }
                }
            }
        ));
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        // Refresh button clicked signal
        self.imp().refresh_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                window.download_pkgbuild();
            }
        ));

        // System color scheme signal
        let style_manager = adw::StyleManager::for_display(&gtk::prelude::WidgetExt::display(self));

        style_manager.connect_dark_notify(clone!(
            #[weak(rename_to = window)] self,
            move |style_manager| {
                let style = if style_manager.is_dark() {
                    "one-dark"
                } else {
                    "one"
                };

                let scheme_manager = sourceview5::StyleSchemeManager::default();

                window.buffer().set_style_scheme(scheme_manager.scheme(style).as_ref());
            }
        ));
    }
}
