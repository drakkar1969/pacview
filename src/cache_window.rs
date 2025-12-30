use std::path::Path;
use std::fmt::Write as _;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use gdk::{Key, ModifierType};

use crate::vars::Pacman;
use crate::cache_object::CacheObject;
use crate::utils::AppInfoExt;

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
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) signature_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
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
        pub(super) footer_label: TemplateChild<gtk::Label>,
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

            obj.setup_signals();
            obj.setup_widgets();
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

            // Show sig files key binding
            klass.add_binding(Key::G, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                imp.signature_button.set_active(!imp.signature_button.is_active());

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
                let cache_file = imp.selection.selected_item()
                    .and_downcast::<CacheObject>()
                    .expect("Failed to downcast to 'CacheObject'")
                    .filename();

                glib::spawn_future_local(async move {
                    AppInfoExt::open_containing_folder(&cache_file).await;
                });
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                let mut output = String::from("## Cache Files\n|File|\n|---|\n");

                for cache in window.imp().selection.iter::<glib::Object>()
                    .flatten()
                    .filter_map(|item| item.downcast::<CacheObject>().ok()) {
                        let _ = writeln!(output, "|{}|", cache.filename());
                    }

                window.clipboard().set_text(&output);
            }
        ));

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.stack.set_visible_child_name(if n_items == 0 { "empty" } else { "view" });

                imp.footer_label.set_label(&format!("{n_items} file{}", if n_items == 1 { "" } else { "s" }));

                imp.copy_button.set_sensitive(n_items > 0);
            }
        ));
    }

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
            let pacman_cache = Pacman::cache().read().unwrap();

            imp.model.splice(0, 0, &pacman_cache.iter()
                .map(|file| CacheObject::new(&file.display().to_string()))
                .collect::<Vec<CacheObject>>()
            );
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
