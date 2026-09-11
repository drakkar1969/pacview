use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::os::unix::fs::MetadataExt;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::{clone, Propagation};
use gio::GioFuture;
use gdk::{Key, ModifierType};

use size::Size;
use walkdir::WalkDir;

use crate::{
    cache_object::CacheObject,
    utils::{Pacman, AppInfoExt}
};

//------------------------------------------------------------------------------
// MODULE: CacheDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::CacheDialog)]
    #[template(resource = "/com/github/PacView/ui/cache_dialog.ui")]
    pub struct CacheDialog {
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
    impl ObjectSubclass for CacheDialog {
        const NAME: &'static str = "CacheDialog";
        type Type = super::CacheDialog;
        type ParentType = adw::Dialog;

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
    impl ObjectImpl for CacheDialog {
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

    impl WidgetImpl for CacheDialog {}
    impl AdwDialogImpl for CacheDialog {}

    impl CacheDialog {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Open action
            klass.install_action_async("cache.open", None, async |dialog, _, _| {
                if let Some(cache_file) = dialog.imp().selection.selected_item()
                    .and_downcast::<CacheObject>() {
                        AppInfoExt::open_containing_folder(&cache_file.path()).await;
                    }
            });

            // Copy action
            klass.install_action("cache.copy", None, |dialog, _, _| {
                let mut output = String::from("## Cache Files\n|File|\n|---|\n");

                for cache in dialog.imp().selection.iter::<glib::Object>()
                    .filter_map(|item| item.ok().and_downcast::<CacheObject>()) {
                        writeln!(output, "|{}|", cache.path()).unwrap();
                    }

                dialog.clipboard().set_text(&output);
            });
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Find key binding
            klass.add_binding(Key::F, ModifierType::CONTROL_MASK, |dialog| {
                dialog.imp().search_bar.set_search_mode(true);

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
// IMPLEMENTATION: CacheDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct CacheDialog(ObjectSubclass<imp::CacheDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl CacheDialog {
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
            #[weak(rename_to = dialog)] self,
            move |selection, _, _, _| {
                let imp = dialog.imp();

                let n_items = selection.n_items();

                imp.stack.set_visible_child_name(
                    if n_items == 0 { "empty" } else { "view" }
                );

                imp.footer_label.set_label(&format!("{n_items} file{}", if n_items == 1 { "" } else { "s" }));

                dialog.action_set_enabled("cache.open", n_items > 0);
                dialog.action_set_enabled("cache.copy", n_items > 0);
            }
        ));

        // Column view activate signal
        imp.view.connect_activate(clone!(
            #[weak(rename_to = dialog)] self,
            move |_, _| {
                dialog.activate_action("cache.open", None).unwrap();
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
            #[weak(rename_to = dialog)] self,
            #[upgrade_or] false,
            move |item| {
                let search_term = dialog.imp().search_term.borrow();

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
    // Populate dialog
    //---------------------------------------
    fn populate(&self) {
        glib::spawn_future_local(clone!(
            #[weak(rename_to = dialog)] self,
            async move {
                let imp = dialog.imp();

                // Spawn future to get cache files and cache size
                let (files, size): (Vec<CacheObject>, u64) = GioFuture::new(&(), |(), _, result| {
                    let mut size = 0;

                    let files: Vec<CacheObject> = Pacman::config().read().unwrap().cache_dir
                        .iter()
                        .flat_map(|dir| {
                            WalkDir::new(dir)
                                .min_depth(1)
                                .sort_by_file_name()
                                .into_iter()
                        })
                        .flatten()
                        .filter_map(|entry| {
                            size += entry.metadata().ok()?.blocks();

                            entry.path().extension().is_some_and(|ext| ext == "zst")
                                .then(|| CacheObject::new(&entry.path().display().to_string()))
                        })
                        .collect();

                    result.resolve((files, size));
                })
                .await;

                imp.model.splice(0, imp.model.n_items(), &files);

                imp.size_label.set_label(
                    &format!("Cache size on disk: {}", Size::from_bytes(size * 512))
                );

                dialog.set_is_loaded(true);
            }
        ));
    }

    //---------------------------------------
    // Present dialog
    //---------------------------------------
    pub fn present(&self, parent: Option<&impl IsA<gtk::Widget>>) {
        adw::Dialog::present(self.upcast_ref::<adw::Dialog>(), parent);

        if !self.is_loaded() {
            self.populate();
        }
    }
}

impl Default for CacheDialog {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
