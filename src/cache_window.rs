use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use crate::window::PACMAN_CACHE;
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
        pub(super) header_sub_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
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

            //---------------------------------------
            // Add class key bindings
            //---------------------------------------
            // Close window binding
            klass.add_binding_action(gdk::Key::Escape, gdk::ModifierType::NO_MODIFIER_MASK, "window.close");

            // Find key binding
            klass.add_binding(gdk::Key::F, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if !imp.search_entry.has_focus() {
                    imp.search_entry.grab_focus();
                }

                glib::Propagation::Stop
            });

            // Open key binding
            klass.add_binding(gdk::Key::O, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.open_button.is_sensitive() {
                    imp.open_button.emit_clicked();
                }

                glib::Propagation::Stop
            });

            // Copy key binding
            klass.add_binding(gdk::Key::C, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.copy_button.is_sensitive() {
                    imp.copy_button.emit_clicked();
                }

                glib::Propagation::Stop
            });
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
    // New function
    //---------------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_entry.set_key_capture_widget(Some(&imp.view.get()));

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
    pub fn show(&self) {
        let imp = self.imp();

        self.present();

        // Populate if necessary
        if imp.model.n_items() == 0 {
            // Get cache files
            let pacman_cache = PACMAN_CACHE.lock().unwrap();

            imp.model.splice(0, 0, &pacman_cache.iter()
                .map(|file| CacheObject::new(&file.display().to_string()))
                .collect::<Vec<CacheObject>>()
            );
        }
    }
}
