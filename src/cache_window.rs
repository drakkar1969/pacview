use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::os::unix::fs::MetadataExt;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, Propagation};
use gdk::{Key, ModifierType};

use size::Size;
use walkdir::WalkDir;

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
        pub(super) search_filter: TemplateChild<gtk::CustomFilter>,

        #[template_child]
        pub(super) footer_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) size_label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        is_loaded: Cell<bool>,

        pub(super) search_term: RefCell<String>,
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
                        writeln!(output, "|{}|", cache.path()).unwrap();
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

        // Search bar search mode enabled signal
        imp.search_bar.connect_search_mode_enabled_notify(clone!(
            #[weak] imp,
            move |bar| {
                if !bar.is_search_mode() {
                    imp.view.grab_focus();
                }
            }
        ));

        // Search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |entry| {
                imp.search_term.replace(entry.text().trim().to_lowercase());

                imp.search_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak(rename_to = window)] self,
            move |selection, _, _, _| {
                let imp = window.imp();

                let n_items = selection.n_items();

                imp.stack.set_visible_child_name(
                    if n_items == 0 { "empty" } else { "view" }
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

        // Set search filter function
        imp.search_filter.set_filter_func(clone!(
            #[weak(rename_to = window)] self,
            #[upgrade_or] false,
            move |item| {
                let search_term = window.imp().search_term.borrow();

                if search_term.is_empty() {
                    return true;
                }

                let obj = item
                    .downcast_ref::<CacheObject>()
                    .expect("Failed to downcast to 'CacheObject'");

                obj.filename().as_bytes()
                    .windows(search_term.len())
                    .any(|window| window.eq_ignore_ascii_case(search_term.as_bytes()))
            }
        ));
        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Populate window
    //---------------------------------------
    fn populate(&self) {
        let imp = self.imp();

        glib::spawn_future_local(clone!(
            #[weak] imp,
            async move {
                // Get cache files and cache size
                let mut cache_size = 0;

                let cache_files: Vec<CacheObject> = Pacman::config().read().unwrap().cache_dir
                    .iter()
                    .flat_map(|dir| {
                        WalkDir::new(dir)
                            .min_depth(1)
                            .sort_by_file_name()
                            .into_iter()
                    })
                    .flatten()
                    .filter_map(|entry| {
                        if let Ok(size) = entry.metadata().map(|metadata| metadata.blocks()) {
                            cache_size += size
                        }

                        entry.path().extension().is_some_and(|ext| ext == "zst")
                            .then(|| CacheObject::new(&entry.path().display().to_string()))
                    })
                    .collect();

                imp.model.splice(0, imp.model.n_items(), &cache_files);

                imp.size_label.set_label(
                    &format!("Cache size on disk: {}", Size::from_bytes(cache_size * 512))
                );
            }
        ));
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self) {
        self.present();

        glib::idle_add_local_once(clone!(
            #[weak(rename_to = window)] self,
            move || {
                if !window.is_loaded() {
                    window.populate();

                    window.set_is_loaded(true);
                }
            }
        ));
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
