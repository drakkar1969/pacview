use std::cell::Cell;
use std::path::Path;
use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::MetadataExt;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, Propagation};
use gdk::{Key, ModifierType};

use size::Size;

use crate::{
    cache_object::CacheObject,
    utils::{Pacman, AppInfoExt}
};

//------------------------------------------------------------------------------
// MODULE: CacheWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::CacheWindow)]
    #[template(resource = "/com/github/PacView/ui/cache_window.ui")]
    pub struct CacheWindow {
        #[template_child]
        pub(super) search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) signature_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,

        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) signature_filter: TemplateChild<gtk::CustomFilter>,

        #[template_child]
        pub(super) footer_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) size_label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        loading: Cell<bool>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for CacheWindow {
        const NAME: &'static str = "CacheWindow";
        type Type = super::CacheWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            CacheObject::ensure_type();

            klass.bind_template();

            // Install actions
            Self::install_actions(klass);

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for CacheWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
            obj.setup_widgets();
        }
    }

    impl WidgetImpl for CacheWindow {}
    impl WindowImpl for CacheWindow {}
    impl AdwWindowImpl for CacheWindow {}

    impl CacheWindow {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Open action
            klass.install_action_async("cache.open", None, async |window, _, _| {
                if let Some(cache_file) = window.imp().selection.selected_item()
                    .and_downcast::<CacheObject>() {
                        AppInfoExt::open_containing_folder(&cache_file.path()).await;
                    }
            });

            // Copy action
            klass.install_action("cache.copy", None, |window, _, _| {
                let mut output = String::from("## Cache Files\n|File|\n|---|\n");

                for cache in window.imp().selection.iter::<glib::Object>()
                    .flatten()
                    .filter_map(|item| item.downcast::<CacheObject>().ok()) {
                        let _ = writeln!(output, "|{}|", cache.path());
                    }

                window.clipboard().set_text(&output);
            });
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Close window binding
            klass.add_binding_action(Key::Escape, ModifierType::NO_MODIFIER_MASK, "window.close");

            // Find key binding
            klass.add_binding(Key::F, ModifierType::CONTROL_MASK, |window| {
                window.imp().search_bar.set_search_mode(true);

                Propagation::Stop
            });

            // Show sig files key binding
            klass.add_binding(Key::G, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                imp.signature_button.set_active(!imp.signature_button.is_active());

                Propagation::Stop
            });

            // Open key binding
            klass.add_binding_action(Key::O, ModifierType::CONTROL_MASK, "cache.open");

            // Copy key binding
            klass.add_binding_action(Key::C, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "cache.copy");
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: CacheWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct CacheWindow(ObjectSubclass<imp::CacheWindow>)
    @extends adw::Window, gtk::Window, gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl CacheWindow {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |entry| {
                imp.search_filter.set_search(Some(&entry.text()));
            }
        ));

        // Signature button toggled signal
        imp.signature_button.connect_toggled(clone!(
            #[weak] imp,
            move |_| {
                imp.signature_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Loading property notify signal
        self.connect_loading_notify(|window| {
            let imp = window.imp();

            imp.stack.set_visible_child_name(
                if imp.selection.n_items() == 0 {
                    if window.loading() { "loading" } else { "empty" }
                } else {
                    "view"
                }
            );
        });

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak(rename_to = window)] self,
            move |selection, _, _, _| {
                let imp = window.imp();

                let n_items = selection.n_items();

                imp.stack.set_visible_child_name(
                    if n_items == 0 {
                        if window.loading() { "loading" } else { "empty" }
                    } else {
                        "view"
                    }
                );

                imp.footer_label.set_label(&format!("{n_items} file{}", if n_items == 1 { "" } else { "s" }));

                window.action_set_enabled("cache.open", n_items > 0);
                window.action_set_enabled("cache.copy", n_items > 0);
            }
        ));

        // Column view activate signal
        imp.view.connect_activate(clone!(
            #[weak(rename_to = window)] self,
            move |_, _| {
                window.activate_action("cache.open", None).unwrap();
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search bar key capture widget
        imp.search_bar.set_key_capture_widget(Some(&imp.view.get()));

        // Bind search button state to search bar visibility
        imp.search_button.bind_property("active", &imp.search_bar.get(), "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();

        // Set signature filter function
        imp.signature_filter.set_filter_func(clone!(
            #[weak] imp,
            #[upgrade_or] false,
            move |item| {
                if imp.signature_button.is_active() {
                    true
                } else {
                    let obj = item
                        .downcast_ref::<CacheObject>()
                        .expect("Failed to downcast to 'CacheObject'");

                    !Path::new(&obj.path())
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("sig"))

                }
            }
        ));

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Populate window
    //---------------------------------------
    pub fn populate(&self) {
        let imp = self.imp();

        // Get cache files
        let cache_files: Vec<CacheObject> = Pacman::cache().read().unwrap().iter()
            .map(|file| CacheObject::new(&file.display().to_string()))
            .collect::<Vec<CacheObject>>();

        imp.model.splice(0, imp.model.n_items(), &cache_files);

        // Get cache size
        let size = 512u64 * Pacman::config().cache_dir.iter()
            .flat_map(|dir| {
                fs::read_dir(dir).into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|entry| {
                        entry.metadata().map(|metadata| metadata.blocks()).ok()
                    })
            })
            .sum::<u64>();

        imp.size_label.set_label(&format!("Cache Size on Disk: {}", Size::from_bytes(size)));

        self.set_loading(false);
    }
}

impl Default for CacheWindow {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
