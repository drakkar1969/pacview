use std::cell::OnceCell;
use std::marker::PhantomData;
use std::fs;
use std::time::Duration;

use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use gdk::{Key, ModifierType};

use sourceview5::prelude::*;

use crate::APP_ID;
use crate::pkg_object::PkgObject;
use crate::utils::{pango_utils, style_schemes, tokio_runtime};

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
        pub(super) save_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) refresh_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) source_view: TemplateChild<sourceview5::View>,
        #[template_child]
        pub(super) error_status: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub(super) url_label: TemplateChild<gtk::Label>,

        #[property(get = Self::buffer)]
        buffer: PhantomData<sourceview5::Buffer>,
        #[property(get, set, construct_only)]
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

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SourceWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
            obj.setup_widgets();
        }
    }

    impl WidgetImpl for SourceWindow {}
    impl WindowImpl for SourceWindow {}
    impl AdwWindowImpl for SourceWindow {}

    impl SourceWindow {
        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Close window binding
            klass.add_binding_action(Key::Escape, ModifierType::NO_MODIFIER_MASK, "window.close");

            // Save binding
            klass.add_binding(Key::S, ModifierType::CONTROL_MASK, |window| {
                window.imp().save_button.emit_clicked();

                glib::Propagation::Stop
            });

            // Refresh binding
            klass.add_binding(Key::F5, ModifierType::NO_MODIFIER_MASK, |window| {
                window.imp().refresh_button.emit_clicked();

                glib::Propagation::Stop
            });
        }

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
        glib::Object::builder()
            .property("transient-for", parent)
            .property("title", format!("{}  \u{2022}  PKGBUILD", &pkg.name()))
            .property("pkg", pkg)
            .build()
    }

    //---------------------------------------
    // Set style scheme helper function
    //---------------------------------------
    fn set_style_scheme(&self, style_manager: &adw::StyleManager) {
        let settings = gio::Settings::new(APP_ID);

        let id = settings.string("pkgbuild-style-scheme");

        let scheme_manager = sourceview5::StyleSchemeManager::default();

        let scheme = (style_schemes::is_variant_dark_by_id(&id) == style_manager.is_dark())
            .then_some(id.clone())
            .or_else(|| style_schemes::variant_id(&id))
            .and_then(|id| scheme_manager.scheme(&id));

        self.buffer().set_style_scheme(scheme.as_ref());
    }

    //---------------------------------------
    // Download PKGBUILD helper function
    //---------------------------------------
    fn download_pkgbuild(&self) {
        let imp = self.imp();

        imp.stack.set_visible_child_name("loading");

        glib::spawn_future_local(clone!(
            #[weak(rename_to = window)] self,
            async move {
                let imp = window.imp();

                // Get PKGBUILD url
                let pkg = window.pkg();
                let (url, raw_url) = pkg.pkgbuild_urls();

                // Set URL label
                imp.url_label.set_label(&format!("<a href=\"{url}\">{url}</a>"));

                // Spawn tokio task to download PKGBUILD
                let result = if raw_url.is_empty() {
                    Err(String::from("PKGBUILD not available"))
                } else if raw_url.starts_with("https://") {
                    tokio_runtime::runtime().spawn(
                        async move {
                            let client = reqwest::Client::builder()
                                .redirect(reqwest::redirect::Policy::none())
                                .build()
                                .map_err(|error| error.to_string())?;

                            let response = client
                                .get(&raw_url)
                                .timeout(Duration::from_secs(5))
                                .send()
                                .await
                                .map_err(|error| error.to_string())?;

                            let status = response.status();

                            if status.is_success() {
                                let pkgbuild = response.text()
                                    .await
                                    .map_err(|error| error.to_string())?;

                                Ok(pkgbuild)
                            } else {
                                Err(status.to_string())
                            }
                        }
                    )
                    .await
                    .expect("Failed to complete tokio task")
                } else {
                    fs::read_to_string(raw_url)
                        .map_err(|error| error.to_string())
                };

                match result {
                    Ok(pkgbuild) => {
                        let buffer = window.buffer();

                        buffer.set_text(&pkgbuild);

                        // Position cursor at start
                        buffer.place_cursor(&buffer.iter_at_offset(0));

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
        let imp = self.imp();

        // System color scheme signal
        let display = gtk::prelude::WidgetExt::display(self);
        let style_manager = adw::StyleManager::for_display(&display);

        style_manager.connect_dark_notify(clone!(
            #[weak(rename_to = window)] self,
            move |style_manager| {
                window.set_style_scheme(style_manager);
            }
        ));

        // Save button clicked signal
        imp.save_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                glib::spawn_future_local(clone!(
                    #[weak] window,
                    async move {
                        let file_dialog = gtk::FileDialog::builder()
                            .modal(true)
                            .title("Save PKGBUILD")
                            .initial_name("PKGBUILD")
                            .build();

                        let response = file_dialog.save_future(Some(&window)).await;

                        if let Ok(file) = response {
                            let source_file = sourceview5::File::new();
                            source_file.set_location(Some(&file));

                            let file_saver = sourceview5::FileSaver::builder()
                                .buffer(&window.buffer())
                                .file(&source_file)
                                .build();

                            let (result, _) = file_saver.save_future(glib::Priority::DEFAULT);

                            let _ = result.await;
                        }
                    }
                ));
            }
        ));

        // Refresh button clicked signal
        imp.refresh_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                window.download_pkgbuild();
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        // Set syntax highlighting language
        let buffer = self.buffer();

        buffer.set_language(
            sourceview5::LanguageManager::default().language("pkgbuild").as_ref()
        );

        // Set style scheme
        let display = gtk::prelude::WidgetExt::display(self);
        let style_manager = adw::StyleManager::for_display(&display);

        self.set_style_scheme(&style_manager);

        // Set font
        let settings = gio::Settings::new(APP_ID);

        let use_system_font = settings.boolean("pkgbuild-use-system-font");
        let mut custom_font = settings.string("pkgbuild-custom-font");

        if use_system_font || custom_font.is_empty() {
            custom_font = style_manager.monospace_font_name();
        }

        let css = pango_utils::font_str_to_css(&custom_font);

        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_string(&format!("textview.card-list {{ {css} }}"));

        gtk::style_context_add_provider_for_display(&display, &css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

        // Download PKGBUILD
        self.download_pkgbuild();
    }
}
