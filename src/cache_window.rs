use std::path::Path;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use gdk::{Key, ModifierType};

use fs_extra::dir;
use size::Size;

use crate::window::{PACMAN_CACHE, PACMAN_CONFIG};
use crate::cache_object::CacheObject;
use crate::utils::app_info;

//------------------------------------------------------------------------------
// MODULE: CacheWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/cache_window.ui")]
    pub struct CacheWindow {
        #[template_child]
        pub(super) header_size_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) header_sub_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) signature_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) signature_filter: TemplateChild<gtk::CustomFilter>,

        #[template_child]
        pub(super) empty_status: TemplateChild<adw::StatusPage>,
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

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CacheWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for CacheWindow {}
    impl WindowImpl for CacheWindow {}
    impl AdwWindowImpl for CacheWindow {}

    impl CacheWindow {
        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Close window binding
            klass.add_binding_action(Key::Escape, ModifierType::NO_MODIFIER_MASK, "window.close");

            // Find key binding
            klass.add_binding(Key::F, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if !imp.search_entry.has_focus() {
                    imp.search_entry.grab_focus();
                }

                glib::Propagation::Stop
            });

            // Open key binding
            klass.add_binding(Key::O, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.open_button.is_sensitive() {
                    imp.open_button.emit_clicked();
                }

                glib::Propagation::Stop
            });

            // Copy key binding
            klass.add_binding(Key::C, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.copy_button.is_sensitive() {
                    imp.copy_button.emit_clicked();
                }

                glib::Propagation::Stop
            });
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
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_entry.set_key_capture_widget(Some(&imp.view.get()));

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

                    !Path::new(&obj.filename())
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("sig"))

                }
            }
        ));

        // Add keyboard shortcut to cancel search
        let shortcut = gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(clone!(
                #[weak] imp,
                #[upgrade_or] glib::Propagation::Proceed,
                move |_, _| {
                    imp.search_entry.set_text("");
                    imp.view.grab_focus();

                    glib::Propagation::Stop
                }
            )))
        );

        let controller = gtk::ShortcutController::new();
        controller.add_shortcut(shortcut);

        imp.search_entry.add_controller(controller);

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search entry search started signal
        imp.search_entry.connect_search_started(|entry| {
            if !entry.has_focus() {
                entry.grab_focus();
            }
        });

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

        // Open button clicked signal
        imp.open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let item = imp.selection.selected_item()
                    .and_downcast::<CacheObject>()
                    .expect("Failed to downcast to 'CacheObject'");

                app_info::open_containing_folder(&item.filename());
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                let body = window.imp().selection.iter::<glib::Object>().flatten()
                    .map(|item| {
                        let cache = item
                            .downcast::<CacheObject>()
                            .expect("Failed to downcast to 'CacheObject'");

                        format!("|{file}|", file=cache.filename())
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                window.clipboard().set_text(&
                    format!("## Cache Files\n|File|\n|---|\n{body}")
                );
            }
        ));

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.empty_status.set_visible(n_items == 0);

                imp.header_sub_label.set_label(&format!("{n_items} file{}", if n_items == 1 { "" } else { "s" }));

                imp.copy_button.set_sensitive(n_items > 0);
            }
        ));
    }

    //---------------------------------------
    // Clear window
    //---------------------------------------
    pub fn remove_all(&self) {
        self.imp().model.remove_all();
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, parent: &impl IsA<gtk::Window>) {
        let imp = self.imp();

        self.present();
        self.set_transient_for(Some(parent));

        // Populate if necessary
        if imp.model.n_items() == 0 {
            // Get cache files
            imp.model.splice(0, 0, &PACMAN_CACHE.lock().unwrap().iter()
                .map(|file| CacheObject::new(&file.display().to_string()))
                .collect::<Vec<CacheObject>>()
            );

            // Get cache size
            let mut cache_size = 0;

            for dir in &PACMAN_CONFIG.cache_dir {
                cache_size += dir::get_size(dir).unwrap_or_default();
            }

            imp.header_size_label.set_label(&Size::from_bytes(cache_size).to_string());
        }
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
